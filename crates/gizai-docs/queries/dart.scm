; Dart symbols. YD-024.
;
; Node names taken from the grammar rather than guessed: a query that names
; a node type the grammar does not have matches nothing, silently, and the
; language looks unsupported rather than mis-queried.
;
; This grammar predates Dart 3's bare `library;` directive, which the
; language registry blanks before parsing. See YD-002.

(class_definition
  name: (identifier) @name) @sym.class

(mixin_declaration
  (identifier) @name) @sym.mixin

(extension_declaration
  name: (identifier) @name) @sym.extension

(enum_declaration
  name: (identifier) @name) @sym.enum

(type_alias
  (type_identifier) @name) @sym.type

; A top-level function, and a method on a class. Dart spells both as a
; signature followed by a body.
(function_signature
  name: (identifier) @name) @sym.function

(method_signature
  (function_signature name: (identifier) @name)) @sym.method

(getter_signature
  name: (identifier) @name) @sym.getter

(setter_signature
  name: (identifier) @name) @sym.setter

(constructor_signature
  name: (identifier) @name) @sym.constructor
