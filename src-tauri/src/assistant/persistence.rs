#[cfg(test)]
mod tests {
    use crate::{assistant::models::AssistantConversation, storage::Store};

    #[test]
    fn schema_v7_restores_visible_conversations_and_projection() {
        let store = Store::open_in_memory(10, 1_024).unwrap();
        let conversation = AssistantConversation::new(
            "conversation-1",
            "session-1",
            "登录接口诊断",
            "2026-11-24T10:00:00Z",
        );

        store.save_assistant_conversation(&conversation).unwrap();
        store
            .save_assistant_projection(
                "conversation-1",
                &serde_json::json!({"messages": [{"role": "assistant", "content": "已完成"}]}),
            )
            .unwrap();

        let restored = store.list_assistant_conversations().unwrap();
        assert_eq!(restored, vec![conversation]);
        assert_eq!(
            store.load_assistant_projection("conversation-1").unwrap(),
            Some(serde_json::json!({"messages": [{"role": "assistant", "content": "已完成"}]}))
        );

        store.hide_assistant_conversation("conversation-1").unwrap();
        assert!(store.list_assistant_conversations().unwrap().is_empty());
    }
}
