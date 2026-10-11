import { describe, expect, it } from 'vitest';
import type { SettingsError } from '../../../../src/modules/project/domain/settings/uigen-settings.js';
import { openFailureToast, settingsErrorMessage } from '../../../../src/renderer/home/open-messages.js';
import { ATLAS_PATH, REFUSED_MESSAGE } from './opening-fixtures.js';

const NOVA_PATH = 'E:\\projets\\nova-ui';
const ORBITE_PATH = 'E:\\clients\\orbite\\ui';
const VITRINE_PATH = 'E:\\documents\\site-vitrine';
const NOT_A_LIT_LIBRARY = {
  tone: 'danger',
  title: "Ce dossier n'est pas une bibliothèque Lit",
  message:
    "Aucun package.json ne déclare la dépendance lit. Rien n'a été modifié. Choisissez le dossier racine de la bibliothèque, ou créez un projet.",
};
const NEWER_FORMAT = {
  tone: 'danger',
  title: "Ce projet ne s'ouvre pas",
  message:
    "uigen.json a été créé par une version plus récente d'UI-GEN. Rien n'a été modifié. Mettez à jour UI-GEN pour ouvrir ce projet.",
};
const FIX_SETTINGS = "Rien n'a été modifié. Corrigez uigen.json, puis ouvrez de nouveau le projet.";

function invalidSettings(path: string, errors: readonly SettingsError[]) {
  return openFailureToast({ kind: 'invalid-settings', path, errors });
}

describe('settingsErrorMessage', () => {
  it('says the file is not valid JSON', () => {
    expect(settingsErrorMessage({ code: 'invalid-json', field: null })).toBe(
      "uigen.json n'est pas un fichier JSON valide.",
    );
  });

  it('says the file must hold a JSON object', () => {
    expect(settingsErrorMessage({ code: 'not-an-object', field: null })).toBe('uigen.json doit contenir un objet JSON.');
  });

  it('names the unknown field as it is written in the file', () => {
    expect(settingsErrorMessage({ code: 'unknown-field', field: 'tagprefix' })).toBe(
      'Champ inconnu « tagprefix » dans uigen.json.',
    );
    expect(settingsErrorMessage({ code: 'unknown-field', field: 'theme' })).toBe('Champ inconnu « theme » dans uigen.json.');
  });

  it('names the missing required field', () => {
    expect(settingsErrorMessage({ code: 'missing-field', field: 'tagPrefix' })).toBe(
      'Champ obligatoire « tagPrefix » absent de uigen.json.',
    );
    expect(settingsErrorMessage({ code: 'missing-field', field: 'componentsDir' })).toBe(
      'Champ obligatoire « componentsDir » absent de uigen.json.',
    );
  });

  it('explains each invalid value', () => {
    expect(settingsErrorMessage({ code: 'invalid-format-version', field: 'formatVersion' })).toBe(
      'formatVersion doit être un entier positif.',
    );
    expect(settingsErrorMessage({ code: 'newer-format-version', field: 'formatVersion' })).toBe(
      "uigen.json a été créé par une version plus récente d'UI-GEN.",
    );
    expect(settingsErrorMessage({ code: 'invalid-tag-prefix', field: 'tagPrefix' })).toBe(
      'tagPrefix commence par une lettre minuscule, ne contient que des lettres minuscules, des chiffres et des tirets, et finit par un tiret.',
    );
    expect(settingsErrorMessage({ code: 'invalid-components-dir', field: 'componentsDir' })).toBe(
      'componentsDir doit être un chemin relatif dans le dossier du projet.',
    );
  });
});

describe('openFailureToast', () => {
  // covers: FCT-0009
  it('explains that a folder without package.json is not a Lit library and that nothing changed', () => {
    expect(openFailureToast({ kind: 'not-a-lit-library', path: VITRINE_PATH, reason: 'missing-package-json' })).toEqual(
      NOT_A_LIT_LIBRARY,
    );
  });

  // covers: FCT-0009
  it('gives the same explanation for an unreadable package.json and for a package without the lit dependency', () => {
    expect(openFailureToast({ kind: 'not-a-lit-library', path: VITRINE_PATH, reason: 'invalid-package-json' })).toEqual(
      NOT_A_LIT_LIBRARY,
    );
    expect(openFailureToast({ kind: 'not-a-lit-library', path: VITRINE_PATH, reason: 'missing-lit-dependency' })).toEqual(
      NOT_A_LIT_LIBRARY,
    );
  });

  it('asks to update UI-GEN when uigen.json was written by a newer version', () => {
    expect(invalidSettings(NOVA_PATH, [{ code: 'newer-format-version', field: 'formatVersion' }])).toEqual(NEWER_FORMAT);
  });

  it('asks to update UI-GEN whatever the place of the newer version among the errors', () => {
    expect(
      invalidSettings(NOVA_PATH, [
        { code: 'unknown-field', field: 'theme' },
        { code: 'invalid-tag-prefix', field: 'tagPrefix' },
        { code: 'newer-format-version', field: 'formatVersion' },
      ]),
    ).toEqual(NEWER_FORMAT);
  });

  it('lists every settings error in the given order, then asks to fix uigen.json', () => {
    expect(
      invalidSettings(ORBITE_PATH, [
        { code: 'unknown-field', field: 'tagprefix' },
        { code: 'missing-field', field: 'tagPrefix' },
      ]),
    ).toEqual({
      tone: 'danger',
      title: "Ce projet ne s'ouvre pas",
      message: `Champ inconnu « tagprefix » dans uigen.json. Champ obligatoire « tagPrefix » absent de uigen.json. ${FIX_SETTINGS}`,
    });
  });

  it('keeps the order of the errors as they are given', () => {
    expect(
      invalidSettings(ORBITE_PATH, [
        { code: 'invalid-tag-prefix', field: 'tagPrefix' },
        { code: 'invalid-components-dir', field: 'componentsDir' },
      ]).message,
    ).toBe(
      `tagPrefix commence par une lettre minuscule, ne contient que des lettres minuscules, des chiffres et des tirets, et finit par un tiret. componentsDir doit être un chemin relatif dans le dossier du projet. ${FIX_SETTINGS}`,
    );
  });

  it('explains a single settings error', () => {
    expect(invalidSettings(ORBITE_PATH, [{ code: 'invalid-json', field: null }])).toEqual({
      tone: 'danger',
      title: "Ce projet ne s'ouvre pas",
      message: `uigen.json n'est pas un fichier JSON valide. ${FIX_SETTINGS}`,
    });
  });

  it('warns that a moved or deleted project cannot be found, naming its folder', () => {
    expect(openFailureToast({ kind: 'missing', path: ATLAS_PATH })).toEqual({
      tone: 'warning',
      title: 'Projet introuvable',
      message: `Le dossier ${ATLAS_PATH} a été déplacé ou supprimé.`,
    });
  });

  it('names the folder and the system code when uigen.json cannot be written', () => {
    expect(openFailureToast({ kind: 'write-failed', path: ATLAS_PATH, code: 'EPERM' })).toEqual({
      tone: 'danger',
      title: "Ce projet ne s'ouvre pas",
      message: `uigen.json n'a pas pu être écrit dans ${ATLAS_PATH} (EPERM). Rien n'a été modifié.`,
    });
  });

  it('says the request was refused when the main process rejects it', () => {
    expect(
      openFailureToast({ kind: 'invalid-request', errors: [{ field: 'tagPrefix', code: 'missing-trailing-dash' }] }),
    ).toEqual({ tone: 'danger', title: "Ce projet ne s'ouvre pas", message: REFUSED_MESSAGE });
  });
});
