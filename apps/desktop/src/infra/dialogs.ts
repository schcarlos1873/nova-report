import { ask, message, open, save } from '@tauri-apps/plugin-dialog';

const NREP = [{ name: 'NovaReport', extensions: ['nrep'] }];

export const dialogs = {
  openReport: async () => {
    const r = await open({ multiple: false, filters: NREP });
    return typeof r === 'string' ? r : null;
  },
  saveReport: (defaultPath?: string) => save({ filters: NREP, defaultPath }),
  saveExport: (ext: string, defaultPath?: string) =>
    save({ defaultPath, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] }),
  pickFile: async (extensions: string[]) => {
    const r = await open({ multiple: false, filters: [{ name: 'Arquivos', extensions }] });
    return typeof r === 'string' ? r : null;
  },
  confirm: (text: string, title = 'NovaReport') => ask(text, { title, kind: 'warning' }),
  error: (text: string) => message(text, { title: 'NovaReport', kind: 'error' }),
};
