; Kotlin symbols. YD-024.
;
; Measured in YD-002: 29 of 40 real files parse clean, the failures being
; Jetpack Compose builder chains. A file with an error still yields the
; symbols from the parts that parsed, so this degrades rather than breaks.

(class_declaration
  (type_identifier) @name) @sym.class

(object_declaration
  (type_identifier) @name) @sym.object

(function_declaration
  (simple_identifier) @name) @sym.function

; Only class members and top-level properties. A bare property_declaration
; also matches every local `val` inside a function body: measured on a real
; Android app, 2,554 of 3,323 matches were locals, which nobody browses
; documentation for.
(class_body
  (property_declaration
    (variable_declaration (simple_identifier) @name)) @sym.property)

(source_file
  (property_declaration
    (variable_declaration (simple_identifier) @name)) @sym.property)

(type_alias
  (type_identifier) @name) @sym.type

(enum_entry
  (simple_identifier) @name) @sym.enum
