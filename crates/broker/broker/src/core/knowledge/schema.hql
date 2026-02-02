schema::1 {
    N::Doc {
        UNIQUE INDEX uri: String,
        epoch: U64,
        seq: U64,
        len_chars: U64,
        language: String,
    }
    N::Chunk {
        INDEX uri: String,
        chunk_idx: U32,
        start_char: U64,
        end_char: U64,
        text: String,
    }
}
