import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useUi } from "../../app/store";
import { Sheet } from "../../ui/primitives";
import { Icon } from "../../ui/Icon";
import appIcon from "../../../src-tauri/icons/128x128@2x.png";
import "./about.css";

const LINKS = [
  { url: "https://zsync.eu", title: "ZSync", description: "The home of ZSync and its other projects." },
  { url: "https://zsync.eu/zpassword-desktop", title: "ZPassword Desktop", description: "This app's page, with news and the latest version." },
  { url: "https://github.com/theholyonez", title: "TheHolyOneZ on GitHub", description: "The developer's profile and other open-source work." },
  { url: "https://github.com/theholyonez/zpassword-desktop", title: "Source code", description: "Read the code, report a bug or suggest a feature." },
];

export function AboutPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const toast = useUi((s) => s.toast);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    if (open && !version) getVersion().then(setVersion).catch(() => setVersion(null));
  }, [open, version]);

  const visit = async (url: string) => {
    try {
      await openUrl(url);
    } catch {
      toast("Couldn't open your browser.", "error");
    }
  };

  return (
    <Sheet open={open} onClose={onClose} title="About" width={600}>
      <div className="about">
        <header className="about__head">
          <img className="about__icon" src={appIcon} alt="" width={72} height={72} />
          <div>
            <h3 className="about__name plate-type">ZPassword Desktop</h3>
            {version && <p className="about__version">Version {version}</p>}
          </div>
        </header>
        <p className="about__desc">
          An offline password vault for Windows and Linux. Your passwords stay in one encrypted file on this computer, with no account and no sync.
        </p>

        <ul className="about__links">
          {LINKS.map((l) => (
            <li key={l.url}>
              <button className="about__link" onClick={() => visit(l.url)}>
                <span className="about__link-text">
                  <span className="about__link-title">{l.title}</span>
                  <span className="about__link-desc">{l.description}</span>
                  <span className="about__link-url code">{l.url.replace("https://", "")}</span>
                </span>
                <Icon name="external" size={16} className="about__link-icon" />
              </button>
            </li>
          ))}
        </ul>

        <p className="about__legal">
          Free software under the GNU General Public License, version 3. Links open in your browser; ZPassword itself never goes online.
        </p>
      </div>
    </Sheet>
  );
}
