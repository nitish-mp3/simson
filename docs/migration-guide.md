# HA VoIP Migration Guide

Migrate to HA VoIP from Asterisk, FreeSWITCH, or a commercial PBX. This guide covers extension mapping, data migration, trunk configuration, and cutover procedures.

---

## Table of Contents

1. [Migration Overview](#1-migration-overview)
2. [From Asterisk](#2-from-asterisk)
3. [From FreeSWITCH](#3-from-freeswitch)
4. [From Commercial PBX](#4-from-commercial-pbx)
5. [Extension Number Mapping](#5-extension-number-mapping)
6. [Data Migration](#6-data-migration)

---

## 1. Migration Overview

### General Approach

1. **Install HA VoIP** alongside the existing PBX (parallel operation).
2. **Create extensions** in HA VoIP that match your current numbering plan.
3. **Configure a SIP trunk** between the existing PBX and HA VoIP.
4. **Test internal calls** between HA VoIP extensions.
5. **Route a subset of traffic** through HA VoIP via the trunk.
6. **Migrate users** one at a time, verifying each.
7. **Cut over** all traffic and decommission the old PBX.

### What Migrates

| Item | Migrated | Notes |
|---|---|---|
| Extension numbers | Yes | Manually recreated |
| Extension passwords | No | Generate new passwords |
| Voicemail messages | Optional | Export as WAV, import via API |
| Call history / CDR | Optional | Export from old PBX, import via SQL |
| IVR / Auto-attendant | No | Rebuild using HA automations |
| Ring groups | Partial | Configure via routing rules |
| Conference bridges | No | Use HA VoIP conference rooms |
| Call recordings | Manual | Copy files, re-index |
| PSTN trunks | Reconfigure | Point SIP trunk to HA VoIP engine |

---

## 2. From Asterisk

### Prerequisites

- Asterisk version 13+ with `pjsip` or `chan_sip` configured.
- Network connectivity between Asterisk and HA VoIP engine.
- List of extensions from `/etc/asterisk/pjsip.conf` or `sip.conf`.

### Step 1: Export Extensions

Extract extensions from Asterisk config:

```bash
# For PJSIP
grep -E "^\[([0-9]+)\]" /etc/asterisk/pjsip.conf | tr -d '[]' > extensions.txt

# For chan_sip
grep -E "^\[([0-9]+)\]" /etc/asterisk/sip.conf | tr -d '[]' > extensions.txt
```

Or from the Asterisk CLI:
```
asterisk -rx "pjsip show endpoints" | awk '/^[0-9]/ {print $1}'
```

### Step 2: Create Extensions in HA VoIP

For each extension, create it via the gRPC API or during the HA integration setup wizard:

```bash
while read ext; do
  grpcurl -plaintext -d "{
    \"number\": \"$ext\",
    \"display_name\": \"Ext $ext\",
    \"password\": \"$(openssl rand -hex 8)\",
    \"transport\": \"wss\",
    \"voicemail_enabled\": true,
    \"max_concurrent_calls\": 2
  }" localhost:50051 voip.VoipService/CreateExtension
done < extensions.txt
```

### Step 3: Configure SIP Trunk (Asterisk -> HA VoIP)

Add a PJSIP trunk in Asterisk:

```ini
; /etc/asterisk/pjsip.conf

[ha-voip-transport]
type = transport
protocol = udp
bind = 0.0.0.0:5060

[ha-voip-trunk]
type = endpoint
context = from-ha-voip
disallow = all
allow = opus
allow = alaw
allow = ulaw
outbound_auth = ha-voip-auth
aors = ha-voip-aor

[ha-voip-auth]
type = auth
auth_type = userpass
username = asterisk-trunk
password = trunk-password-here

[ha-voip-aor]
type = aor
contact = sip:192.168.1.50:5060   ; HA VoIP engine IP

[ha-voip-identify]
type = identify
endpoint = ha-voip-trunk
match = 192.168.1.50
```

Add a corresponding routing rule in HA VoIP to route calls to Asterisk:

```bash
grpcurl -plaintext -d '{
  "pattern": "^[2-9][0-9]{9}$",
  "destination": "sip:192.168.1.100:5060",
  "priority": 10,
  "description": "Route PSTN calls to Asterisk"
}' localhost:50051 voip.VoipService/SetRoutingRule
```

### Step 4: Configure Dialplan (Asterisk)

Route calls to HA VoIP extensions through the trunk:

```ini
; /etc/asterisk/extensions.conf

[from-ha-voip]
; Incoming calls from HA VoIP
exten => _X.,1,NoOp(Call from HA VoIP: ${EXTEN})
exten => _X.,n,Dial(PJSIP/${EXTEN})
exten => _X.,n,Hangup()

[to-ha-voip]
; Route to HA VoIP extensions
exten => _1XX,1,NoOp(Routing to HA VoIP ext ${EXTEN})
exten => _1XX,n,Dial(PJSIP/${EXTEN}@ha-voip-trunk)
exten => _1XX,n,Hangup()
```

### Step 5: Migrate Voicemail

Export voicemail messages from Asterisk:

```bash
# Asterisk stores voicemails in /var/spool/asterisk/voicemail/
for ext_dir in /var/spool/asterisk/voicemail/default/*/INBOX/*.wav; do
  ext=$(basename $(dirname $(dirname "$ext_dir")))
  echo "Extension $ext: $ext_dir"
done
```

Import into HA VoIP via the gRPC API (audio as bytes).

### Step 6: Cutover

1. Update PSTN SIP trunk registrations to point to HA VoIP instead of Asterisk.
2. Change DNS SRV records if applicable.
3. Re-register all SIP phones to the HA VoIP engine.
4. Monitor for 48 hours, then decommission Asterisk.

---

## 3. From FreeSWITCH

### Prerequisites

- FreeSWITCH with XML config in `/etc/freeswitch/`.
- List of users from `/etc/freeswitch/directory/default/`.

### Step 1: Export Users

```bash
ls /etc/freeswitch/directory/default/*.xml | \
  xargs grep -h 'id=' | \
  sed 's/.*id="\([^"]*\)".*/\1/' > extensions.txt
```

Or via `fs_cli`:
```
fs_cli -x "list_users"
```

### Step 2: Create Extensions

Same as [Asterisk Step 2](#step-2-create-extensions-in-ha-voip).

### Step 3: Configure SIP Trunk (FreeSWITCH -> HA VoIP)

Create a gateway in FreeSWITCH:

```xml
<!-- /etc/freeswitch/sip_profiles/external/ha-voip.xml -->
<include>
  <gateway name="ha-voip">
    <param name="username" value="freeswitch-trunk"/>
    <param name="password" value="trunk-password"/>
    <param name="realm" value="homeassistant.local"/>
    <param name="proxy" value="192.168.1.50:5060"/>
    <param name="register" value="false"/>
    <param name="caller-id-in-from" value="true"/>
  </gateway>
</include>
```

### Step 4: Dialplan

```xml
<!-- Route 1XX extensions to HA VoIP -->
<extension name="to-ha-voip">
  <condition field="destination_number" expression="^(1\d{2})$">
    <action application="bridge" data="sofia/gateway/ha-voip/$1"/>
  </condition>
</extension>
```

### Step 5: Migrate CDR

FreeSWITCH CDRs are typically stored in `/var/log/freeswitch/cdr-csv/` or a database.

```bash
# Export CSV CDRs
cat /var/log/freeswitch/cdr-csv/Master.csv | \
  awk -F',' '{print $3","$4","$5","$12","$13}' > cdr_export.csv
```

Import into HA VoIP's database (see [Data Migration](#6-data-migration)).

---

## 4. From Commercial PBX

### General Steps

Commercial PBX systems (Cisco UCM, Avaya, Mitel, etc.) typically provide a web admin interface for exporting data.

1. **Export extension list** from the PBX admin panel (usually CSV).
2. **Export CDR** if available (CSV or database export).
3. **Note PSTN trunk settings:** SIP provider, credentials, codec preferences.
4. **Create extensions** in HA VoIP using the exported list.
5. **Configure PSTN trunk** to point to HA VoIP.
6. **Test and cutover.**

### Cisco UCM

Export from Cisco UCM Bulk Administration:
1. Navigate to **Bulk Administration > Phones > Export Phones**.
2. Download the CSV.
3. Extract the Directory Number (DN) and Display Name columns.

### Avaya

Export from Avaya System Manager:
1. Navigate to **User Management > Manage Users > Export**.
2. Download the CSV.
3. Map the "Extension" and "Display Name" fields.

### Mitel / ShoreTel

Export from the MiVoice Connect admin:
1. Navigate to **Users > Export**.
2. Map extension numbers and names.

### Mapping Considerations

| PBX Feature | HA VoIP Equivalent |
|---|---|
| Extension | Extension (1:1 mapping) |
| Hunt group | Routing rule with priority |
| Auto attendant | HA automation with TTS |
| Call park | Not yet supported (use hold/transfer) |
| Paging | HA automation + media_player entity |
| Intercom | Auto-answer call |

---

## 5. Extension Number Mapping

### Planning

Create a mapping table before migration:

| Old Number | New Number | Name | Department | Notes |
|---|---|---|---|---|
| 2001 | 100 | Alice Smith | Engineering | Primary phone |
| 2002 | 101 | Bob Jones | Sales | + mobile |
| 2003 | 102 | Reception | Admin | Auto-answer |
| 9100 | 200 | Conf Room A | -- | Conference |

### Renumbering Considerations

- If keeping the same numbering plan, no renumbering is needed.
- If renumbering, publish the old-to-new mapping and allow a transition period.
- Consider using a routing rule on the old PBX to forward old numbers to the new system during transition.

### DID Mapping

For PSTN Direct Inward Dialing (DID):

| DID Number | Old Extension | New Extension |
|---|---|---|
| +1-555-100-0001 | 2001 | 100 |
| +1-555-100-0002 | 2002 | 101 |

Configure routing rules in HA VoIP:
```bash
grpcurl -plaintext -d '{
  "pattern": "^\\+15551000001$",
  "destination": "100",
  "priority": 1,
  "description": "DID -> Alice"
}' localhost:50051 voip.VoipService/SetRoutingRule
```

---

## 6. Data Migration

### Call History / CDR

#### Export Format

Prepare a CSV with the following columns:
```csv
call_id,from_number,to_number,start_time,end_time,duration_sec,answered,direction
```

Example:
```csv
legacy-001,100,101,2024-01-15T10:30:00Z,2024-01-15T10:35:00Z,300,true,outbound
legacy-002,102,100,2024-01-15T11:00:00Z,2024-01-15T11:00:30Z,30,false,inbound
```

#### Import Script

```python
#!/usr/bin/env python3
"""Import legacy CDR into HA VoIP database."""

import csv
import sqlite3
from datetime import datetime

DB_PATH = "/var/lib/voip-engine/voip-engine.db"
CSV_PATH = "cdr_export.csv"

conn = sqlite3.connect(DB_PATH)
cursor = conn.cursor()

with open(CSV_PATH) as f:
    reader = csv.DictReader(f)
    for row in reader:
        cursor.execute(
            """INSERT INTO call_history
               (call_id, from_uri, to_uri, started_at, ended_at,
                duration_sec, answered, direction, migrated)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)""",
            (
                row["call_id"],
                f"sip:{row['from_number']}@homeassistant.local",
                f"sip:{row['to_number']}@homeassistant.local",
                row["start_time"],
                row["end_time"],
                int(row["duration_sec"]),
                row["answered"].lower() == "true",
                row["direction"],
            ),
        )

conn.commit()
conn.close()
print(f"Imported {reader.line_num - 1} CDR records")
```

### Voicemail Messages

1. Export voicemails as WAV or Opus files from the old PBX.
2. Import them via the gRPC `CreateVoicemail` API:

```python
import grpc
from voip_pb2 import CreateVoicemailRequest
from voip_pb2_grpc import VoipServiceStub

channel = grpc.insecure_channel("localhost:50051")
stub = VoipServiceStub(channel)

with open("vm-100-001.wav", "rb") as f:
    audio = f.read()

stub.CreateVoicemail(CreateVoicemailRequest(
    extension_id="ext-100",
    caller_id="sip:unknown@external",
    audio_data=audio,
    duration_sec=45,
))
```

### Contact / Directory Data

HA VoIP does not maintain a separate contact directory -- it uses Home Assistant's person entities and the configured extension display names. To import a contact list:

1. Create HA person entities for each user.
2. Map extensions to persons in the VoIP card configuration.

### Recordings

Copy recording files to the HA VoIP recordings directory:
```bash
cp /var/spool/asterisk/monitor/*.wav /var/lib/voip-engine/recordings/
```

If the engine uses Opus format, convert with ffmpeg:
```bash
for f in /old-recordings/*.wav; do
  ffmpeg -i "$f" -c:a libopus -b:a 32k \
    "/var/lib/voip-engine/recordings/$(basename "$f" .wav).opus"
done
```

### Rollback Plan

Keep the old PBX running for at least 2 weeks after cutover:

1. Do not decommission the old PBX immediately.
2. Maintain the SIP trunk between old and new systems.
3. If issues arise, redirect the PSTN trunk back to the old PBX.
4. Only decommission after a full business cycle (typically 1-2 weeks) without issues.
