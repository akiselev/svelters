this.compile = (filename, src) =>
    this.svelte
        .preprocess(
            src,
            {
                script: ({ content, filename }) => {
                    const code = this.transpile(filename, content);
                    return { code };
                },
            },
            { filename },
        )
        .then(({ code }) => {
            const res = this.svelte.compile(code, {
                namespace: "foreign",
                accessors: true,
                filename: filename,
                name: "Component",
            }).js.code;
            return res;
        });
