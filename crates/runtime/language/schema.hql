schema::2 {
    N::Language {
        UNIQUE INDEX name: String,
        idx: U32,
    }

    N::LangExtension {
        INDEX extension: String,
        idx: U32,
    }

    N::LangFilename {
        INDEX filename: String,
        idx: U32,
    }

    N::LangShebang {
        INDEX shebang: String,
        idx: U32,
    }
}
