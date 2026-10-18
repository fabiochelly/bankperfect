# Installation : brew install --cask fabiochelly/bankperfect/bankperfect
# (le dépôt sert de « tap » : brew tap fabiochelly/bankperfect https://github.com/fabiochelly/bankperfect)
cask "bankperfect" do
  version "9.8.0"
  sha256 "61ad4d7836dfaab2420ab562f3be9d4a10eceff1eaf8268f4edf0a85a99fd5d4"

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
