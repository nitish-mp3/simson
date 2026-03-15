//! Call recording module.
//!
//! Manages active recordings, writes audio to disk as WAV files, encrypts
//! completed recordings with AES-256-GCM, and tracks metadata.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use chrono::Utc;
use dashmap::DashMap;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::RecordingConfig;

// ───────────────────── Errors ─────────────────────

#[derive(Debug, Error)]
pub enum RecordingError {
    #[error("No active recording for call {0}")]
    NotRecording(String),
    #[error("Recording {0} not found")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Disk quota exceeded")]
    DiskQuotaExceeded,
}

// ───────────────────── Metadata ─────────────────────

/// Metadata for a completed recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub recording_id: String,
    pub call_id: String,
    pub file_path: String,
    pub duration_secs: f64,
    pub size_bytes: u64,
    pub encrypted: bool,
    pub participants: Vec<String>,
    pub created_at: String,
}

// ───────────────────── Active recording ─────────────────────

/// An in-progress recording.
pub struct ActiveRecording {
    pub call_id: String,
    pub file_path: PathBuf,
    pub started_at: Instant,
    /// Async file handle for streaming writes.
    pub writer: tokio::fs::File,
    /// AES-256-GCM key (32 bytes).
    pub encryption_key: [u8; 32],
    /// Nonce for AES-256-GCM (12 bytes).
    pub nonce: [u8; 12],
    /// Codec clock rate.
    sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo).
    channels: u16,
    /// Running count of PCM bytes written (pre-encryption).
    pcm_bytes_written: u64,
    /// Participants in the recording.
    participants: Vec<String>,
}

// ───────────────────── Recording manager ─────────────────────

/// Manages active and completed recordings.
pub struct RecordingManager {
    pub active_recordings: DashMap<String, ActiveRecording>,
    pub config: RecordingConfig,
    /// Completed recording metadata, keyed by recording_id.
    completed: DashMap<String, RecordingMetadata>,
}

impl RecordingManager {
    /// Create a new recording manager.
    pub fn new(config: RecordingConfig) -> Self {
        RecordingManager {
            active_recordings: DashMap::new(),
            config,
            completed: DashMap::new(),
        }
    }

    /// Start recording a call.
    ///
    /// Creates the output directory if needed, opens a temporary PCM file,
    /// and returns a `recording_id`.
    pub async fn start_recording(
        &self,
        call_id: &str,
        participants: Vec<String>,
    ) -> Result<String, RecordingError> {
        // Check disk quota.
        if self.config.max_disk_mb > 0 {
            let usage = self.calculate_disk_usage().await;
            let limit = self.config.max_disk_mb * 1024 * 1024;
            if usage >= limit {
                warn!(usage, limit, "Disk quota exceeded");
                return Err(RecordingError::DiskQuotaExceeded);
            }
        }

        let recording_id = Uuid::new_v4().to_string();
        let dir = self.config.directory.join(call_id);
        fs::create_dir_all(&dir).await?;

        let file_name = format!("{recording_id}.pcm.tmp");
        let file_path = dir.join(&file_name);
        let writer = fs::File::create(&file_path).await?;

        let mut key = [0u8; 32];
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut key);
        rand::thread_rng().fill_bytes(&mut nonce);

        let recording = ActiveRecording {
            call_id: call_id.to_string(),
            file_path: file_path.clone(),
            started_at: Instant::now(),
            writer,
            encryption_key: key,
            nonce,
            sample_rate: 8000,
            channels: 1,
            pcm_bytes_written: 0,
            participants: participants.clone(),
        };

        self.active_recordings
            .insert(call_id.to_string(), recording);

        info!(call_id, recording_id = %recording_id, "Recording started");
        Ok(recording_id)
    }

    /// Write an RTP payload frame to the recording.
    ///
    /// `direction` is informational ("tx" or "rx").
    pub async fn write_audio(
        &self,
        call_id: &str,
        rtp_payload: &[u8],
        _direction: &str,
    ) -> Result<(), RecordingError> {
        let mut rec = self
            .active_recordings
            .get_mut(call_id)
            .ok_or_else(|| RecordingError::NotRecording(call_id.into()))?;

        rec.writer.write_all(rtp_payload).await?;
        rec.pcm_bytes_written += rtp_payload.len() as u64;

        Ok(())
    }

    /// Stop recording and finalise the file.
    ///
    /// 1. Flush and close the temporary PCM file.
    /// 2. Wrap the PCM data in a WAV container.
    /// 3. Optionally encrypt the WAV file.
    /// 4. Write the final file and remove the temp.
    pub async fn stop_recording(
        &self,
        call_id: &str,
    ) -> Result<RecordingMetadata, RecordingError> {
        let (_, mut recording) = self
            .active_recordings
            .remove(call_id)
            .ok_or_else(|| RecordingError::NotRecording(call_id.into()))?;

        recording.writer.flush().await?;
        recording.writer.shutdown().await?;

        // Read back the raw PCM data.
        let pcm_data = fs::read(&recording.file_path).await?;

        // Build WAV.
        let wav_data = build_wav(
            &pcm_data,
            recording.sample_rate,
            recording.channels,
        );

        let duration = recording.pcm_bytes_written as f64
            / (recording.sample_rate as f64
                * recording.channels as f64
                * 2.0); // 16-bit = 2 bytes

        // Determine final path.
        let final_name = recording
            .file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("recording")
            .trim_end_matches(".pcm");
        let dir = recording.file_path.parent().unwrap_or(Path::new("."));

        let (final_path, encrypted) = if self.config.encrypt {
            let enc_path = dir.join(format!("{final_name}.wav.enc"));
            let enc_data = encrypt_file_data(&wav_data, &recording.encryption_key)?;
            fs::write(&enc_path, &enc_data).await?;
            (enc_path, true)
        } else {
            let wav_path = dir.join(format!("{final_name}.wav"));
            fs::write(&wav_path, &wav_data).await?;
            (wav_path, false)
        };

        // Remove the temp file.
        let _ = fs::remove_file(&recording.file_path).await;

        let size = fs::metadata(&final_path).await?.len();

        let metadata = RecordingMetadata {
            recording_id: final_name.to_string(),
            call_id: call_id.to_string(),
            file_path: final_path.to_string_lossy().to_string(),
            duration_secs: duration,
            size_bytes: size,
            encrypted,
            participants: recording.participants,
            created_at: Utc::now().to_rfc3339(),
        };

        self.completed
            .insert(metadata.recording_id.clone(), metadata.clone());

        info!(
            call_id,
            duration_secs = duration,
            size_bytes = size,
            encrypted,
            "Recording saved"
        );

        Ok(metadata)
    }

    /// Encrypt a file on disk with AES-256-GCM.
    pub async fn encrypt_file(
        path: &Path,
        key: &[u8; 32],
    ) -> Result<(), RecordingError> {
        let data = fs::read(path).await?;
        let encrypted = encrypt_file_data(&data, key)?;
        let enc_path = path.with_extension("enc");
        fs::write(&enc_path, &encrypted).await?;
        info!(path = %enc_path.display(), "File encrypted");
        Ok(())
    }

    /// Retrieve metadata for a completed recording.
    pub fn get_recording(&self, recording_id: &str) -> Result<RecordingMetadata, RecordingError> {
        self.completed
            .get(recording_id)
            .map(|r| r.clone())
            .ok_or_else(|| RecordingError::NotFound(recording_id.into()))
    }

    /// Delete a completed recording (file + metadata).
    pub async fn delete_recording(&self, recording_id: &str) -> Result<(), RecordingError> {
        let (_, meta) = self
            .completed
            .remove(recording_id)
            .ok_or_else(|| RecordingError::NotFound(recording_id.into()))?;

        let path = Path::new(&meta.file_path);
        if path.exists() {
            fs::remove_file(path).await?;
        }

        info!(recording_id, "Recording deleted");
        Ok(())
    }

    /// Calculate the total disk usage of the recordings directory in bytes.
    pub async fn calculate_disk_usage(&self) -> u64 {
        calculate_dir_size(&self.config.directory).await
    }

    /// Check whether a call is currently being recorded.
    pub fn is_recording(&self, call_id: &str) -> bool {
        self.active_recordings.contains_key(call_id)
    }
}

// ───────────────────── WAV header generation ─────────────────────

/// Build a complete WAV file from raw 16-bit PCM data.
fn build_wav(pcm_data: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = pcm_data.len() as u32;
    let file_size = 36 + data_size; // RIFF size = file - 8 header bytes

    let mut wav = Vec::with_capacity(44 + pcm_data.len());

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt sub-chunk (16 bytes for PCM)
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // sub-chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data sub-chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm_data);

    wav
}

// ───────────────────── PCM mixing ─────────────────────

/// Mix two 16-bit PCM mono streams into one.
///
/// Samples are summed with saturation to prevent wrap-around clipping.
/// If streams differ in length, the shorter one is zero-padded.
pub fn mix_pcm_streams(stream_a: &[i16], stream_b: &[i16]) -> Vec<i16> {
    let len = stream_a.len().max(stream_b.len());
    let mut mixed = Vec::with_capacity(len);

    for i in 0..len {
        let a = if i < stream_a.len() {
            stream_a[i] as i32
        } else {
            0
        };
        let b = if i < stream_b.len() {
            stream_b[i] as i32
        } else {
            0
        };
        mixed.push((a + b).clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    }

    mixed
}

/// Convert a slice of `i16` samples to a byte vector (little-endian).
pub fn samples_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

/// Convert a byte slice of little-endian 16-bit PCM to `Vec<i16>`.
pub fn bytes_to_samples(data: &[u8]) -> Vec<i16> {
    data.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

// ───────────────────── AES-256-GCM encryption ─────────────────────

/// Encrypt `data` with AES-256-GCM.  The output is `nonce(12) || ciphertext`.
fn encrypt_file_data(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, RecordingError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| RecordingError::Encryption(e.to_string()))?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = AesNonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| RecordingError::Encryption(e.to_string()))?;

    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    Ok(out)
}

/// Decrypt data produced by `encrypt_file_data`.
pub fn decrypt_file_data(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, RecordingError> {
    if encrypted.len() < 12 {
        return Err(RecordingError::Encryption("Data too short".into()));
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = AesNonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| RecordingError::Encryption(e.to_string()))?;

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| RecordingError::Encryption(e.to_string()))
}

// ───────────────────── Helpers ─────────────────────

/// Recursively calculate directory size in bytes.
async fn calculate_dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(mut entries) = fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(meta) = entry.metadata().await {
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() {
                    total += Box::pin(calculate_dir_size(&entry.path())).await;
                }
            }
        }
    }
    total
}

// ───────────────────── Tests ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_structure() {
        let pcm = vec![0u8; 1000];
        let wav = build_wav(&pcm, 8000, 1);

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 1000);

        // Verify sample rate in header (bytes 24-27, little-endian).
        let sr = u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]);
        assert_eq!(sr, 8000);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let data = b"Hello, VoIP recording!";

        let encrypted = encrypt_file_data(data, &key).unwrap();
        assert_ne!(&encrypted[12..], data.as_slice());

        let decrypted = decrypt_file_data(&encrypted, &key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn decrypt_wrong_key() {
        let key = [42u8; 32];
        let wrong = [99u8; 32];
        let enc = encrypt_file_data(b"secret", &key).unwrap();
        assert!(decrypt_file_data(&enc, &wrong).is_err());
    }

    #[test]
    fn mix_pcm_basic() {
        let a = vec![100i16, 200, 300, 400];
        let b = vec![50, 100, 150, 200];
        let mixed = mix_pcm_streams(&a, &b);
        assert_eq!(mixed, vec![150, 300, 450, 600]);
    }

    #[test]
    fn mix_pcm_clipping() {
        let a = vec![i16::MAX, i16::MIN];
        let b = vec![1000, -1000];
        let mixed = mix_pcm_streams(&a, &b);
        assert_eq!(mixed[0], i16::MAX);
        assert_eq!(mixed[1], i16::MIN);
    }

    #[test]
    fn mix_pcm_different_lengths() {
        let a = vec![100i16, 200];
        let b = vec![50, 100, 150, 200];
        let mixed = mix_pcm_streams(&a, &b);
        assert_eq!(mixed.len(), 4);
        assert_eq!(mixed[2], 150);
    }

    #[test]
    fn samples_bytes_roundtrip() {
        let samples = vec![100i16, -200, 32767, -32768, 0];
        let bytes = samples_to_bytes(&samples);
        let recovered = bytes_to_samples(&bytes);
        assert_eq!(recovered, samples);
    }
}
