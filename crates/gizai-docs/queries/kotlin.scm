; Kotlin symbols, for tree-sitter-kotlin-ng (whose nodes differ from the grammar yodocs read: `identifier` where it had
; `type_identifier` and `simple_identifier`). An interface or an enum is a class_declaration: kind detection tells them
; apart by their header. Members (methods, properties, enum entries) are inside their class's text.

(class_declaration
  name: (_) @name) @sym.class

(object_declaration
  name: (_) @name) @sym.object

(source_file
  (function_declaration
    name: (_) @name) @sym.function)

(source_file
  (property_declaration
    (variable_declaration (identifier) @name)) @sym.constant)

(type_alias
  (identifier) @name) @sym.type
