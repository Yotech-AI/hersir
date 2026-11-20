; Dart symbols, for tree-sitter-dart 0.3 (whose nodes differ from the grammar yodocs read). A class extending a Flutter
; widget is a widget by kind detection. Methods are inside their class's text.

(class_declaration
  name: (_) @name) @sym.class

(mixin_declaration
  name: (_) @name) @sym.mixin

(extension_declaration
  name: (_) @name) @sym.extension

(enum_declaration
  name: (_) @name) @sym.enum

(function_declaration
  signature: (function_signature
    name: (_) @name)) @sym.function

(type_alias
  (type_identifier) @name) @sym.type

; const and final: a static_final_declaration_list; var: an initialized_identifier_list
(top_level_variable_declaration
  (static_final_declaration_list
    (static_final_declaration
      name: (_) @name))) @sym.constant

(top_level_variable_declaration
  (initialized_identifier_list
    (initialized_identifier
      name: (_) @name))) @sym.constant
