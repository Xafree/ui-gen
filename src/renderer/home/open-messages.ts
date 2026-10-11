import type { OpenProjectOutcome } from '../../modules/project/application/open/project-outcomes.js';
import type { SettingsError } from '../../modules/project/domain/settings/uigen-settings.js';
import type { ToastRequest } from '../motion/toast.js';
import { REQUEST_REFUSED } from '../studio-notices.js';

export type OpenFailure = Exclude<OpenProjectOutcome, { readonly kind: 'opened' } | { readonly kind: 'needs-adoption' }>;

const CANNOT_OPEN = "Ce projet ne s'ouvre pas";
const NOTHING_CHANGED = "Rien n'a été modifié.";
const UPDATE_STUDIO = 'Mettez à jour UI-GEN pour ouvrir ce projet.';
const FIX_SETTINGS = 'Corrigez uigen.json, puis ouvrez de nouveau le projet.';

export function settingsErrorMessage(error: SettingsError): string {
  const field = error.field ?? '';
  switch (error.code) {
    case 'invalid-json':
      return "uigen.json n'est pas un fichier JSON valide.";
    case 'not-an-object':
      return 'uigen.json doit contenir un objet JSON.';
    case 'unknown-field':
      return `Champ inconnu « ${field} » dans uigen.json.`;
    case 'missing-field':
      return `Champ obligatoire « ${field} » absent de uigen.json.`;
    case 'invalid-format-version':
      return 'formatVersion doit être un entier positif.';
    case 'newer-format-version':
      return "uigen.json a été créé par une version plus récente d'UI-GEN.";
    case 'invalid-tag-prefix':
      return 'tagPrefix commence par une lettre minuscule, ne contient que des lettres minuscules, des chiffres et des tirets, et finit par un tiret.';
    case 'invalid-components-dir':
      return 'componentsDir doit être un chemin relatif dans le dossier du projet.';
  }
}

function invalidSettingsMessage(errors: readonly SettingsError[]): string {
  const newer = errors.find((error) => error.code === 'newer-format-version');
  if (newer !== undefined) return `${settingsErrorMessage(newer)} ${NOTHING_CHANGED} ${UPDATE_STUDIO}`;
  return `${errors.map(settingsErrorMessage).join(' ')} ${NOTHING_CHANGED} ${FIX_SETTINGS}`;
}

export function openFailureToast(outcome: OpenFailure): ToastRequest {
  switch (outcome.kind) {
    case 'not-a-lit-library':
      return {
        tone: 'danger',
        title: "Ce dossier n'est pas une bibliothèque Lit",
        message: `Aucun package.json ne déclare la dépendance lit. ${NOTHING_CHANGED} Choisissez le dossier racine de la bibliothèque, ou créez un projet.`,
      };
    case 'invalid-settings':
      return { tone: 'danger', title: CANNOT_OPEN, message: invalidSettingsMessage(outcome.errors) };
    case 'missing':
      return {
        tone: 'warning',
        title: 'Projet introuvable',
        message: `Le dossier ${outcome.path} a été déplacé ou supprimé.`,
      };
    case 'write-failed':
      return {
        tone: 'danger',
        title: CANNOT_OPEN,
        message: `uigen.json n'a pas pu être écrit dans ${outcome.path} (${outcome.code}). ${NOTHING_CHANGED}`,
      };
    case 'invalid-request':
      return { tone: 'danger', title: CANNOT_OPEN, message: REQUEST_REFUSED.message };
  }
}
