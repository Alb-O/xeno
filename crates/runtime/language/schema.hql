schema::1 {
    N::Language {
        UNIQUE INDEX name: String,
        INDEX extension_idx: String,
        INDEX filename_idx: String,
        INDEX shebang_idx: String,
        idx: U32,
    }
}
