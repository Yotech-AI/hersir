; TypeScript symbols. YD-023.
; Shared shape with tsx.scm and javascript.scm: whole declaration as
; @sym.<kind>, its name as @name.

(class_declaration
  name: (type_identifier) @name) @sym.class

(abstract_class_declaration
  name: (type_identifier) @name) @sym.class

(interface_declaration
  name: (type_identifier) @name) @sym.interface

(type_alias_declaration
  name: (type_identifier) @name) @sym.type

(enum_declaration
  name: (identifier) @name) @sym.enum

(function_declaration
  name: (identifier) @name) @sym.function

(generator_function_declaration
  name: (identifier) @name) @sym.function

; `const f = () => {}` and `const f = function () {}` read as functions,
; because that is what they are to anyone looking for one.
(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: [(arrow_function) (function_expression)])) @sym.function

; A plain exported constant. Config objects and lookup tables live here.
(export_statement
  (lexical_declaration
    (variable_declarator
      name: (identifier) @name
      value: [(object) (array) (string) (number) (call_expression)]))) @sym.constant

(method_definition
  name: (property_identifier) @name) @sym.method

(public_field_definition
  name: (property_identifier) @name) @sym.property

(import_statement
  source: (string) @name) @sym.import
