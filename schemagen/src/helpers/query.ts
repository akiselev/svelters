export const root = (q: string) => `(translation_unit (${q}))`;
export const preproc = (q: string) => `(preproc_ifdef (${q}))`;

export const findQtNamespace = `
(
    (declaration) @ns
    (#match? @ns "QT_BEGIN_NAMESPACE")
)
`;

export const findQtWidgets = `
(
    (class_specifier
        (type_identifier) @class
        (base_class_clause (type_identifier)+ @base)
    ) @ns
    (#match? @class "^Q[A-Z][a-zA-Z]+")
)
`;

export const ex = `
(
    (identifier) @constant
    (#match? @constant "^[A-Z][A-Z_]+")
)
`;
