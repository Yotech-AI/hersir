# hersir: Hersir from the ready-made macOS build of one of its releases (https://github.com/Yotech-AI/hersir), for Macs
# with Apple silicon.
# - The archive is the release's own, as Release builds made it. Its sha256 is its line in the release's SHA256SUMS,
#   which the release's signature (hersir-X.Y.Z.sig) covers, with the commit the release's tag points to.
# - A formula, never a cask: Homebrew puts the quarantine mark on what a cask downloads, and Hersir.app is signed ad hoc,
#   not by Apple, so macOS would stop it at its first start. Nothing here sets or removes that mark.
# - scripts/packages.sh in Yotech-AI/hersir fills in the url and the sha256 for each signed release, from
#   packaging/homebrew/hersir.rb there, and publishes nothing else: changes to this file go there.
# These packaging files are under 0BSD (LICENSE). Hersir itself is under the Apache License 2.0.
class Hersir < Formula
  desc "Talk to one Chief that runs a team of AI coding agents through your board"
  homepage "https://hersir.dev"
  url "https://github.com/Yotech-AI/hersir/releases/download/v0.18.1/hersir-0.18.1-macos-arm64.tar.gz"
  sha256 "dc3fe6f9af4293376d9702a5d180314244c9ec1979808ea7fc007567bfac62f0"
  license "Apache-2.0"

  livecheck do
    url :stable
    strategy :github_latest
  end

  depends_on arch: :arm64
  depends_on :macos

  def install
    # Homebrew unpacks the archive and goes into its one folder, target/: the build is in release/.
    app = buildpath/"release/bundle/macos/Hersir.app"
    # hersir-mcp, the helper the Chief's chat starts, goes into the app beside hersir, where Hersir looks for it. Then
    # the app is signed ad hoc again, as install.sh does, so its seal covers hersir-mcp too: Apple silicon runs only
    # signed programs. An ad hoc signature names no one.
    (app/"Contents/MacOS").install "release/hersir-mcp"
    system "/usr/bin/codesign", "--force", "--deep", "--sign", "-", app
    prefix.install app
    # The hersir command starts the app's own program, so macOS shows it as Hersir and Hersir finds hersir-mcp beside
    # it. Not a link: macOS tells a program the path of the link it was started from.
    bin.write_exec_script prefix/"Hersir.app/Contents/MacOS/hersir"
    bin.install_symlink prefix/"Hersir.app/Contents/MacOS/hersir-mcp"
  end

  def caveats
    <<~EOS
      To open Hersir from Finder, Spotlight and the Dock, link it into Applications in your home folder, once:
        mkdir -p ~/Applications && ln -sfn #{opt_prefix}/Hersir.app ~/Applications/Hersir.app
      Or start it with: hersir

      Hersir doesn't update itself when Homebrew installed it. Update it with
        brew update && brew upgrade hersir
      then restart Hersir. brew uninstall hersir keeps your data in ~/Library/Application Support/Hersir.
    EOS
  end

  test do
    assert_equal "hersir #{version}", shell_output("#{bin}/hersir --version").strip
  end
end
