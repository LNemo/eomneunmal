use std::time::Instant;

use eomneunmal::platform::macos::{
    accessibility_trusted_for_current_process, capture_focused_text_snapshot_with_chat_history,
};

fn main() {
    println!(
        "accessibility_trusted={}",
        accessibility_trusted_for_current_process()
    );
    match capture_focused_text_snapshot_with_chat_history(Instant::now()) {
        Some(snapshot) => {
            println!(
                "target={}; app_id={}; role={}; protected={}; text_len={}; empty={}; message_input={}; description_present={}; placeholder_present={}; identifier_hash_present={}; chat_history_hash_present={}",
                snapshot.target.label(),
                snapshot.app_id,
                snapshot.role,
                snapshot.is_protected,
                snapshot.text.chars().count(),
                snapshot.is_empty(),
                snapshot.looks_like_editable_chat_input(),
                snapshot.description.is_some(),
                snapshot.placeholder.is_some(),
                snapshot.identifier_hash.is_some(),
                snapshot.chat_history_hash.is_some()
            );
        }
        None => {
            println!("no supported focused text snapshot");
        }
    }
}
