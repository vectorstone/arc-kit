class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/vectorstone/arc-kit"
  license "MIT"
  version "2026.5.24.2"

  on_macos do
    on_arm do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.5.24.2/arc-kit-aarch64-apple-darwin.tar.gz"
      sha256 "37ad88be476b81de4ea8689a1ba159fc19e11476f658eb216b7a8919c933ad45"
    end

    on_intel do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.5.24.2/arc-kit-x86_64-apple-darwin.tar.gz"
      sha256 "36791f6ea6a2183d4a2907c976f016f3dee47d9d58d3b28f027a28f9cc749d39"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.5.24.2/arc-kit-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "7c5ae7aeb23ee1875ca55e53209e96f4dd44126ccedfdcc4a691153f58749015"
    end

    on_intel do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.5.24.2/arc-kit-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "a585e478bd9d6ce519ec4d674bb90754ecb4914b8d5a0023f0d092871097b363"
    end
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
