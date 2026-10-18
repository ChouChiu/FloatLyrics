// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { AppIcon, Button, Card } from "../components/ui";
import { sendUiAction, type UiState } from "../ui-store";
import { translator } from "../ui-text";

const PROJECT_URL = "https://github.com/ChouChiu/FloatLyrics";
const LYRICSX_URL = "https://github.com/MxIris-LyricsX-Project/LyricsX";

export function AboutPage({ state }: { state: UiState }) {
  const t = translator(state);
  return (
    <div className="page-stack about-content">
      <Card className="hero-card">
        <AppIcon className="app-mark" />
        <h1>FloatLyrics</h1>
        <p>{t("AppSummary")}</p>
        <span>
          {t("Version")} {state.version}
        </span>
        <Button
          variant="outline"
          onClick={() => sendUiAction({ type: "open-url", url: PROJECT_URL })}
        >
          {t("ProjectWebsite")}
        </Button>
      </Card>
      <Card>
        <div className="card-intro">
          <h2>{t("AcknowledgementsTitle")}</h2>
          <p>{t("InspiredByLyricsX")}</p>
          <Button
            variant="ghost"
            onClick={() => sendUiAction({ type: "open-url", url: LYRICSX_URL })}
          >
            {t("VisitLyricsX")}
          </Button>
        </div>
      </Card>
      <Card>
        <div className="card-intro">
          <h2>{t("OpenSourceTitle")}</h2>
          <p>{t("OpenSourceDescription")}</p>
        </div>
        <div className="dependency-grid">
          {state.about.dependencies.map((dependency) => (
            <div className="dependency" key={`${dependency.name}-${dependency.version}`}>
              <strong>{dependency.name}</strong>
              <span>
                v{dependency.version} · {dependency.license}
              </span>
            </div>
          ))}
        </div>
        <h3>{t("LicenseTexts")}</h3>
        {state.about.licenses.map((license) => (
          <details key={`${license.name}-${license.id}`}>
            <summary>
              {license.name} · {license.id}
            </summary>
            <pre>{license.text}</pre>
          </details>
        ))}
      </Card>
    </div>
  );
}
