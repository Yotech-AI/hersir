; PHP symbols. YD-022.
;
; Every pattern captures the whole declaration as @sym.<kind> and its name as
; @name. The runner reads those two captures and nothing else, so adding a
; kind here needs no code change.
;
; Verified against a real Laravel app/ folder: 40 of 40 files parse clean.

(class_declaration
  name: (name) @name) @sym.class

(interface_declaration
  name: (name) @name) @sym.interface

(trait_declaration
  name: (name) @name) @sym.trait

(enum_declaration
  name: (name) @name) @sym.enum

(function_definition
  name: (name) @name) @sym.function

(method_declaration
  name: (name) @name) @sym.method

; Promoted constructor properties are not captured: they belong to the
; constructor's signature, which is already part of the method entry.
(property_declaration
  (property_element
    (variable_name (name) @name))) @sym.property

(const_declaration
  (const_element (name) @name)) @sym.constant

; Namespace and imports. Both are literal text in the file, which is what
; makes kind detection possible without resolving anything across files:
; `use Illuminate\Database\Eloquent\Model;` plus `extends Model` is enough.
(namespace_definition
  name: (namespace_name) @name) @sym.namespace

(namespace_use_declaration
  (namespace_use_clause
    (qualified_name) @name)) @sym.use
