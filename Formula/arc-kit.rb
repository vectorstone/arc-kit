class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/vectorstone/arc-kit"
  license "MIT"
  version "2026.5.24"

  on_macos do
    on_arm do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.5.24/arc-kit-aarch64-apple-darwin.tar.gz"
      sha256 "9bbb81c0062d9d22a4f19d898be4a8dd9ac0e72692715aee14c9e612e0668387"
    end

    on_intel do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.5.24/arc-kit-x86_64-apple-darwin.tar.gz"
      sha256 "76b04996b8f96d276d6c9b26e1b30853887083bddc55d28f8ee3e5ac40414653"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.5.24/arc-kit-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "cf773960fa98ba4ef07c57d6cd9e8c2325aea519864adab11ad393ef1bdf2a73"
    end

    on_intel do
      url "https://github.com/vectorstone/arc-kit/releases/download/v2026.5.24/arc-kit-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "baf669ad1ce797879be59a8281b744855c482bc5372f0e8fbceb98a083f541f6"
    end
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
