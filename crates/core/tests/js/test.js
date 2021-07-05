export const testReturnValue = (val = "World") => `Hello ${val}`;

export const testConsole = (val = "Hello World") => console.log("Test:", val);

export const testArgs = (...args) => {
    return args.join(" ");
};
