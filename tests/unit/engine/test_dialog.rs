//! Unit tests for the SIP dialog state machine.
//!
//! Verifies dialog lifecycle (creation, state transitions, termination),
//! transaction retransmission behaviour, dialog matching via Call-ID + tags,
//! and concurrent dialog handling.

#[cfg(test)]
mod tests {
    use crate::sip::dialog::{Dialog, DialogState, Transaction, TransactionState};

    // ───────────────────────── Helper factories ─────────────────────────

    fn make_dialog(call_id: &str, local_tag: &str, remote_tag: &str) -> Dialog {
        Dialog::new(
            call_id.to_string(),
            local_tag.to_string(),
            remote_tag.to_string(),
            "sip:alice@atlanta.example.com".to_string(),
            "sip:bob@biloxi.example.com".to_string(),
        )
    }

    fn make_invite_transaction(branch: &str) -> Transaction {
        Transaction::new_client_invite(
            branch.to_string(),
            "sip:bob@biloxi.example.com".to_string(),
        )
    }

    // ═══════════════════════════════════════════════════════════════════
    //  1. Dialog creation
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_dialog_created_in_trying_state() {
        let dlg = make_dialog("call-1@atlanta", "ltag1", "");
        assert_eq!(dlg.state(), DialogState::Trying);
        assert_eq!(dlg.call_id(), "call-1@atlanta");
        assert_eq!(dlg.local_tag(), "ltag1");
    }

    #[test]
    fn test_dialog_has_correct_uris() {
        let dlg = make_dialog("call-2@atlanta", "ltag2", "");
        assert_eq!(dlg.local_uri(), "sip:alice@atlanta.example.com");
        assert_eq!(dlg.remote_uri(), "sip:bob@biloxi.example.com");
    }

    // ═══════════════════════════════════════════════════════════════════
    //  2. State transitions
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_trying_to_proceeding() {
        let mut dlg = make_dialog("call-3@atlanta", "ltag3", "");
        assert_eq!(dlg.state(), DialogState::Trying);

        dlg.on_provisional_response(180, "rtag3");
        assert_eq!(dlg.state(), DialogState::Proceeding);
        assert_eq!(dlg.remote_tag(), "rtag3");
    }

    #[test]
    fn test_proceeding_to_confirmed() {
        let mut dlg = make_dialog("call-4@atlanta", "ltag4", "");
        dlg.on_provisional_response(180, "rtag4");
        assert_eq!(dlg.state(), DialogState::Proceeding);

        dlg.on_success_response(200);
        assert_eq!(dlg.state(), DialogState::Confirmed);
    }

    #[test]
    fn test_confirmed_to_terminated_via_bye() {
        let mut dlg = make_dialog("call-5@atlanta", "ltag5", "");
        dlg.on_provisional_response(180, "rtag5");
        dlg.on_success_response(200);
        assert_eq!(dlg.state(), DialogState::Confirmed);

        dlg.on_bye();
        assert_eq!(dlg.state(), DialogState::Terminated);
    }

    #[test]
    fn test_trying_to_terminated_on_reject() {
        let mut dlg = make_dialog("call-6@atlanta", "ltag6", "");
        assert_eq!(dlg.state(), DialogState::Trying);

        // A 486 Busy Here terminates the dialog immediately
        dlg.on_error_response(486);
        assert_eq!(dlg.state(), DialogState::Terminated);
    }

    #[test]
    fn test_proceeding_to_terminated_on_cancel() {
        let mut dlg = make_dialog("call-7@atlanta", "ltag7", "");
        dlg.on_provisional_response(180, "rtag7");
        assert_eq!(dlg.state(), DialogState::Proceeding);

        dlg.on_cancel();
        assert_eq!(dlg.state(), DialogState::Terminated);
    }

    // ═══════════════════════════════════════════════════════════════════
    //  3. Transaction retransmission
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_transaction_retransmit_interval_doubles() {
        let mut txn = make_invite_transaction("z9hG4bKretrans");
        assert_eq!(txn.state(), TransactionState::Calling);

        // RFC 3261 17.1.1.2: Timer A fires at T1, 2*T1, 4*T1, ...
        let t1 = txn.timer_t1_ms();
        let first_interval = txn.next_retransmit_interval_ms();
        assert_eq!(first_interval, t1);

        txn.record_retransmit();
        let second_interval = txn.next_retransmit_interval_ms();
        assert_eq!(second_interval, t1 * 2);

        txn.record_retransmit();
        let third_interval = txn.next_retransmit_interval_ms();
        assert_eq!(third_interval, t1 * 4);
    }

    #[test]
    fn test_transaction_max_retransmits() {
        let mut txn = make_invite_transaction("z9hG4bKmaxretrans");
        // RFC 3261: Timer B (transaction timeout) = 64*T1
        let max_retransmits = txn.max_retransmits();
        assert!(
            max_retransmits >= 6 && max_retransmits <= 11,
            "INVITE client transaction should allow 6-11 retransmissions"
        );

        // Exhaust all retransmissions
        for _ in 0..max_retransmits {
            assert!(!txn.is_terminated());
            txn.record_retransmit();
        }
        txn.on_timeout();
        assert_eq!(txn.state(), TransactionState::Terminated);
    }

    // ═══════════════════════════════════════════════════════════════════
    //  4. Dialog matching
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_dialog_matches_by_call_id_and_tags() {
        let dlg = make_dialog("call-8@atlanta", "ltag8", "rtag8");
        assert!(dlg.matches("call-8@atlanta", "ltag8", "rtag8"));
        assert!(!dlg.matches("call-8@atlanta", "ltag8", "wrong_tag"));
        assert!(!dlg.matches("wrong-id@atlanta", "ltag8", "rtag8"));
    }

    // ═══════════════════════════════════════════════════════════════════
    //  5. Concurrent dialogs
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_concurrent_dialogs_independent() {
        let mut dlg_a = make_dialog("call-A@atlanta", "ltA", "rtA");
        let mut dlg_b = make_dialog("call-B@atlanta", "ltB", "rtB");

        dlg_a.on_provisional_response(180, "rtA");
        dlg_a.on_success_response(200);
        assert_eq!(dlg_a.state(), DialogState::Confirmed);
        assert_eq!(dlg_b.state(), DialogState::Trying);

        dlg_b.on_error_response(603);
        assert_eq!(dlg_a.state(), DialogState::Confirmed);
        assert_eq!(dlg_b.state(), DialogState::Terminated);
    }
}
