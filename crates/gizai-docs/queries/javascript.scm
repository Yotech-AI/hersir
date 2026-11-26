; JavaScript symbols. YD-023. TypeScript's patterns minus the type-only ones.

(class_declaration
  name: (identifier) @name) @sym.class

(function_declaration
  name: (identifier) @name) @sym.function

(generator_function_declaration
  name: (identifier) @name) @sym.function

(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: [(arrow_function) (function_expression)])) @sym.function

(export_statement
  (lexical_declaration
    (variable_declarator
      name: (identifier) @name
      value: [(object) (array) (string) (number) (call_expression)]))) @sym.constant

(method_definition
  name: (property_identifier) @name) @sym.method

(import_statement
  source: (string) @name) @sym.import
