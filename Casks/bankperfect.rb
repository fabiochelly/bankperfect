# Installation : brew install --cask fabiochelly/bankperfect/bankperfect
# (le dépôt sert de « tap » : brew tap fabiochelly/bankperfect https://github.com/fabiochelly/bankperfect)
cask "bankperfect" do
  version "9.5.0"
  sha256 "61ceded9a8acd1e1d5afa2722ceb97853f92bb9efa60db86a46531f1c1485d2e"

  url "https://github.com/fabiochelly/bankperfect/releases/download/v#{version}/BankPerfect-#{version}-macos-universal.zip"
  name "BankPerfect"
  desc "Gestion de comptes bancaires personnels"
  homepage "https://github.com/fabiochelly/bankperfect"

  depends_on macos: ">= :big_sur"

  app "BankPerfect.app"

  # Application non signée par Apple : on retire la quarantaine pour que
  # Gatekeeper la laisse s'ouvrir.
  postflight do
    system_command "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "#{appdir}/BankPerfect.app"]
  end

  zap trash: [
    "~/Library/Application Support/net.chelly.bankperfect",
    "~/Library/Preferences/net.chelly.bankperfect",
  ]
end
