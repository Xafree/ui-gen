import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { makeTempFolder, removeFolder } from "../../project/studio-app";
import {
  canvasContent,
  openRecentProject,
  rememberOpenedComponent,
} from "../canvas/canvas-project";
import {
  PRICING_CARD_FILE,
  withStudio,
  writeKitHorizon,
} from "../editor-project";
import { writeKitHorizonSupport, writeTsconfig } from "../screen/editor-screen";
import { clickRow, nodeIdOf } from "../selection/selection-project";

const WARNING_MESSAGE = "Expression non représentable";
const NOT_REPRESENTABLE_MESSAGE =
  "Cette expression ne peut pas être affichée dans l'éditeur visuel. Elle est conservée dans le code.";
const EXPRESSION_TEXT = "${formatPrice(this.price, this.locale)}";
const CTA_FIRST_LINE = 159;
const CTA_LAST_LINE = 169;
const LOCALE_PROPERTY = "@property() locale = 'fr-FR';";
const LOCALE_PROPERTY_WITHOUT_VALUE = "@property() locale = ;";

let root: string;
let userData: string;
let project: string;

test.beforeEach(async () => {
  root = await makeTempFolder("code-panel");
  userData = await makeTempFolder("code-panel-user-data");
  project = join(root, "kit-horizon");
  await writeKitHorizon(project);
  await writeTsconfig(project);
  await writeKitHorizonSupport(project);
  await rememberOpenedComponent(userData, project, "hz-pricing-card");
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

function underlines(window: Page): Locator {
  return window.locator("#code .ui-diag");
}

function codeLines(window: Page): Locator {
  return window.locator("#code .ui-code__line");
}

async function openPricingCard(window: Page): Promise<void> {
  await openRecentProject(window);
  await expect(
    canvasContent(window).locator("hz-pricing-card").first(),
  ).toBeAttached();
  await expect(underlines(window).first()).toBeVisible();
}

async function targetLineNumbers(window: Page): Promise<number[]> {
  return codeLines(window).evaluateAll(
    (lines: { classList: { contains(name: string): boolean } }[]) =>
      lines.flatMap((line, index) =>
        line.classList.contains("is-target") ? [index + 1] : [],
      ),
  );
}

// covers: FCT-0039
test("underlines the expression the visual editor cannot show and counts it as one warning", async () => {
  await withStudio(userData, async ({ window }) => {
    await openPricingCard(window);

    await expect(underlines(window)).toHaveCount(1);
    await expect(underlines(window)).toHaveClass(/ui-diag--warning/);
    await expect(underlines(window)).toHaveText(EXPRESSION_TEXT);
    await expect(window.locator("#problem-count")).toHaveText("1");
    await expect(window.locator("#status-warn-label")).toHaveText(
      "1 avertissement",
    );
  });
});

// covers: FCT-0039
test("shows the message, the line and the column of the expression when hovering its underline", async () => {
  await withStudio(userData, async ({ window }) => {
    await openPricingCard(window);

    await expect(async () => {
      await window.mouse.move(0, 0);
      await underlines(window).first().hover();
      await expect(window.locator("#diag-title")).toHaveText(WARNING_MESSAGE, {
        timeout: 2000,
      });
      await expect(window.locator("#diag-text")).toHaveText(
        NOT_REPRESENTABLE_MESSAGE,
        { timeout: 2000 },
      );
      await expect(window.locator("#diag-meta")).toHaveText(
        "hz-pricing-card.ts, ligne 152, col. 11 · ui-gen(visual-sync)",
        { timeout: 2000 },
      );
    }).toPass({ timeout: 20_000 });
  });
});

// covers: FCT-0043
test("flags an unreadable file and keeps the last readable canvas when the syntax breaks on disk", async () => {
  await withStudio(userData, async ({ window }) => {
    await openPricingCard(window);
    const instances = canvasContent(window).locator("hz-pricing-card");
    const countBefore = await instances.count();
    expect(countBefore).toBeGreaterThan(0);
    const file = join(project, PRICING_CARD_FILE);
    const readable = await readFile(file, "utf8");
    expect(readable).toContain(LOCALE_PROPERTY);

    await writeFile(
      file,
      readable.replace(LOCALE_PROPERTY, LOCALE_PROPERTY_WITHOUT_VALUE),
      "utf8",
    );

    await expect(window.locator("#code-sync")).toHaveText(
      "Code illisible : le canvas garde la dernière version lisible",
      {
        timeout: 15_000,
      },
    );
    await expect(window.locator("#status-sync-label")).toHaveText(
      "Erreur de syntaxe",
    );
    await expect(instances).toHaveCount(countBefore);
    await expect(
      canvasContent(window).locator("hz-pricing-card .plan").first(),
    ).toBeAttached();
  });
});

// covers: FCT-0036
test("highlights the code lines of the layer selected in the tree and only them", async () => {
  await withStudio(userData, async ({ window }) => {
    await openPricingCard(window);
    const instance = canvasContent(window).locator("hz-pricing-card").first();
    const variant = await instance.getAttribute("variant");
    const id = `${variant ?? ""}:${await nodeIdOf(instance.locator("hz-button.cta"))}`;
    expect(await targetLineNumbers(window)).toEqual([]);

    await clickRow(window, id);

    await expect
      .poll(() => targetLineNumbers(window))
      .toEqual(
        Array.from(
          { length: CTA_LAST_LINE - CTA_FIRST_LINE + 1 },
          (_, offset) => CTA_FIRST_LINE + offset,
        ),
      );
  });
});
