this.compile = (filename, src) =>
    svelte
        .preprocess(
            src,
            {
                script: ({ content, filename }) => {
                    const code = transpile(filename, content);
                    return { code };
                },
            },
            { filename },
        )
        .then(({ code }) => {
            const res = svelte.compile(code, {
                namespace: "foreign",
                accessors: true,
                filename: filename,
                name: "Component",
            }).js.code;
            return res;
        });
