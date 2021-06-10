this.compileSvelteComponent = () => {
    const { code } = this.svelte.preprocess(this.source, {
        script: ({ content }) => this.transform(content),
    });
    return this.svelte.compile(code, {
        namespace: "foreign",
        accessors: true,
        filename: this.filename,
        name: this.name,
    }).js.code;
};
