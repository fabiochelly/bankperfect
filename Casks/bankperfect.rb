# Installation : brew install --cask fabiochelly/bankperfect/bankperfect
# (le dépôt sert de « tap » : brew tap fabiochelly/bankperfect https://github.com/fabiochelly/bankperfect)
cask "bankperfect" do
  version "9.7.1"
  sha256 "64a17df6e1e8ec31131b63681c2bc8d66f2d52a4287a50674a177438647c1822"

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
