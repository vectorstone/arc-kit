class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/vectorstone/arc-kit"
  license "MIT"
  version "2026.6.12"

  on_macos do
    on_arm do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.6.12/arc-kit-aarch64-apple-darwin.tar.gz"
      sha256 "b040db18b76d920ea0f553ff1475500ad5caeb0b6115d56cb516dd183b94d26c"
    end

    on_intel do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.6.12/arc-kit-x86_64-apple-darwin.tar.gz"
      sha256 "fe60fc630b0cbedeef0ba1cece886670da7835dfdb804ada80dc0c7ada1ae550"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.6.12/arc-kit-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "4af4f777385884760b7c6fd451510c602e4439d141e7608b7086b930da36cabb"
    end

    on_intel do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.6.12/arc-kit-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "0a78616a1a11e1a6cd12b4adc8e61fa9317499b8e536fde816b566f97be5a5d8"
    end
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
