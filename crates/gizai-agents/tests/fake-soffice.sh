#!/bin/sh
# GA-311 QA: a fake LibreOffice (soffice) for the tests of the PDF copies (libreoffice_test.rs in gizai-agents,
# lead_files_test.rs and chat_files_flow_test.rs in src-tauri). It takes what Hersir passes:
#   soffice --headless --norestore -env:UserInstallation=file://<profile> --convert-to pdf --outdir <dir> <file>
# Each call is logged in the profile folder (made when missing, as LibreOffice does), in call-<pid>: its arguments, one
# per line, then PATH=<its PATH>. What it does depends on the first line of the file it converts:
#   FAKE_FAIL   says "Error: source file could not be loaded" on stderr (after two warnings) and exits 1, with no PDF
#   FAKE_SLOW   starts a child that sleeps (its pid in <profile>/child, its own in <profile>/parent) and sleeps itself
#   FAKE_LEAVE  makes the PDF, starts a child that sleeps (its pid in <profile>/child) and exits 0
#   FAKE_NOPDF  exits 0 without a PDF
#   FAKE_HALF   writes half a PDF (no %%EOF) and exits 1
#   FAKE_WAIT   makes the PDF after 0.3 s
#   anything else: makes <name without its extension>.pdf in <dir>, a small whole PDF with the file's first line in a
#   comment, and exits 0.
# While it works, <profile>/busy is there; a fake that finds it there adds its pid to <profile>/overlap.
profile=
for a in "$@"; do
  case "$a" in
    -env:UserInstallation=file://*) profile="${a#-env:UserInstallation=file://}" ;;
  esac
done
mkdir -p "$profile"
{ printf '%s\n' "$@"; printf 'PATH=%s\n' "$PATH"; } > "$profile/call-$$"

outdir=
src=
while [ $# -gt 0 ]; do
  case "$1" in
    --outdir) shift; outdir="$1" ;;
    --convert-to) shift ;;
    -*) ;;
    *) src="$1" ;;
  esac
  shift
done

if [ -e "$profile/busy" ]; then echo "$$" >> "$profile/overlap"; fi
echo "$$" > "$profile/busy"

first=$(head -n 1 "$src")
base=$(basename "$src")
pdf="$outdir/${base%.*}.pdf"

whole_pdf() {
  printf '%%PDF-1.4\n%% %s\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj\ntrailer<</Root 1 0 R>>\n%%%%EOF\n' "$first" > "$pdf"
}

case "$first" in
  FAKE_FAIL*)
    echo "javaldx: Could not find a Java Runtime Environment!" >&2
    echo "Warning: failed to read path from javaldx" >&2
    echo "Error: source file could not be loaded" >&2
    rm -f "$profile/busy"
    exit 1 ;;
  FAKE_SLOW*)
    sleep 300 &
    echo "$!" > "$profile/child"
    echo "$$" > "$profile/parent"
    sleep 300
    exit 0 ;;
  FAKE_LEAVE*)
    whole_pdf
    sleep 300 &
    echo "$!" > "$profile/child"
    rm -f "$profile/busy"
    exit 0 ;;
  FAKE_NOPDF*)
    rm -f "$profile/busy"
    exit 0 ;;
  FAKE_HALF*)
    printf '%%PDF-1.4\n1 0 obj<</Type/Catalog' > "$pdf"
    rm -f "$profile/busy"
    exit 1 ;;
  FAKE_WAIT*)
    sleep 0.3
    whole_pdf ;;
  *)
    whole_pdf ;;
esac
echo "convert $src -> $pdf using filter : fake_pdf_Export"
rm -f "$profile/busy"
exit 0
