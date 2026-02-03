schema::1 {
    N::Doc {
        UNIQUE INDEX uri: String,
        epoch: U64,
        seq: U64,
        len_chars: U64,
        language: String,
        mtime: U64,
    }
    N::Chunk {
        INDEX doc_uri: String,
        chunk_idx: U32,
        start_char: U64,
        end_char: U64,
        text: String,
    }
    N::SharedDoc {
        UNIQUE INDEX uri: String,
        epoch: U64,
        seq: U64,
        len_chars: U64,
        hash64: U64,
        head_node_id: U64,
        root_node_id: U64,
        next_node_id: U64,
        history_nodes: U64,
    }
    N::HistoryNode {
        UNIQUE INDEX node_key: String,
        INDEX doc_uri: String,
        node_id: U64,
        parent_id: U64,
        redo_tx: String,
        undo_tx: String,
        len_chars: U64,
        hash64: U64,
        is_root: Boolean,
        root_text: String,
    }
}
