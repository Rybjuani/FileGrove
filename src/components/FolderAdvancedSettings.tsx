import { useEffect, useMemo, useState } from 'react';
import { ChevronDown, ChevronUp, Settings2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { DestinationMode, Folder, MaxDepth, SubfolderConfig } from '../ipc/types';
import { isDangerousAction } from '../lib/rules';
import { useAppStore } from '../stores/useAppStore';

export function FolderAdvancedSettings({ folder }: { folder: Folder }) {
  const { t } = useTranslation();
  const { rules, updateFolderConfiguration } = useAppStore();
  const [open, setOpen] = useState(false);
  const [recursive, setRecursive] = useState(folder.subfolders.enabled);
  const [destinationMode, setDestinationMode] = useState<DestinationMode>(
    folder.subfolders.destinationMode,
  );
  const [maxDepth, setMaxDepth] = useState(depthText(folder.subfolders.maxDepth));
  const [excludeFolders, setExcludeFolders] = useState(folder.excludeFolders.join('\n'));
  const [excludePatterns, setExcludePatterns] = useState(folder.excludePatterns.join('\n'));
  const [ruleIds, setRuleIds] = useState(folder.ruleIds);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    setRecursive(folder.subfolders.enabled);
    setDestinationMode(folder.subfolders.destinationMode);
    setMaxDepth(depthText(folder.subfolders.maxDepth));
    setExcludeFolders(folder.excludeFolders.join('\n'));
    setExcludePatterns(folder.excludePatterns.join('\n'));
    setRuleIds(folder.ruleIds);
  }, [folder]);

  const enabledRules = useMemo(
    () => [...rules].filter((rule) => rule.enabled).sort((a, b) => a.order - b.order),
    [rules],
  );

  const toggleRule = (ruleId: string) => {
    setRuleIds((current) =>
      current.includes(ruleId)
        ? current.filter((candidate) => candidate !== ruleId)
        : [...current, ruleId],
    );
    setSaved(false);
  };

  const save = async () => {
    const nextDepth: MaxDepth =
      maxDepth === 'unlimited' ? 'unlimited' : Math.max(1, Math.min(64, Number(maxDepth) || 1));
    const folders = normalizedLines(excludeFolders);
    const patterns = normalizedLines(excludePatterns);
    const newlyAssignedDangerous = enabledRules.filter(
      (rule) =>
        ruleIds.includes(rule.id) &&
        !folder.ruleIds.includes(rule.id) &&
        isDangerousAction(rule.action),
    );
    const automationActive =
      folder.mode === 'auto' || (folder.mode === 'scheduled' && Boolean(folder.schedule?.enabled));
    if (
      automationActive &&
      newlyAssignedDangerous.length > 0 &&
      !window.confirm(
        t('folderCard.advanced.dangerousAssignmentConfirm', {
          count: newlyAssignedDangerous.length,
        }),
      )
    ) {
      return;
    }

    const config: SubfolderConfig = {
      enabled: folder.mode === 'auto' ? false : recursive,
      destinationMode,
      firstRunConfirmed: folder.subfolders.firstRunConfirmed,
      maxDepth: nextDepth,
    };
    setSaving(true);
    const success = await updateFolderConfiguration(folder.id, config, folders, patterns, ruleIds);
    setSaving(false);
    setSaved(success);
  };

  return (
    <section className="rounded-md border border-primary-800 bg-primary-950/40">
      <button
        type="button"
        className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
      >
        <span className="flex items-center gap-2 text-xs font-medium text-primary-300">
          <Settings2 size={14} /> {t('folderCard.advanced.title')}
        </span>
        {open ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
      </button>

      {open && (
        <div className="space-y-5 border-t border-primary-800 p-3">
          <section className="space-y-2">
            <p className="text-xs font-medium text-primary-300">
              {t('folderCard.advanced.assignedRules')}
            </p>
            {enabledRules.length === 0 ? (
              <p className="text-xs text-primary-500">{t('folderCard.advanced.noEnabledRules')}</p>
            ) : (
              <div className="grid gap-2 sm:grid-cols-2">
                {enabledRules.map((rule) => (
                  <label
                    key={rule.id}
                    className="flex items-start gap-2 rounded border border-primary-800 p-2 text-xs text-primary-300"
                  >
                    <input
                      type="checkbox"
                      checked={ruleIds.includes(rule.id)}
                      onChange={() => toggleRule(rule.id)}
                    />
                    <span>
                      {rule.name}
                      {isDangerousAction(rule.action) && (
                        <span className="ml-2 text-red-300">
                          {t(`rules.action.${rule.action.type}`)}
                        </span>
                      )}
                    </span>
                  </label>
                ))}
              </div>
            )}
            {enabledRules.some(
              (rule) => ruleIds.includes(rule.id) && isDangerousAction(rule.action),
            ) && (
              <p className="rounded bg-yellow-500/10 p-2 text-xs text-yellow-200">
                {t('folderCard.advanced.dangerousRuleWarning')}
              </p>
            )}
          </section>

          <section className="space-y-3">
            <label className="flex items-start gap-2 text-sm text-primary-300">
              <input
                type="checkbox"
                checked={recursive}
                disabled={folder.mode === 'auto'}
                onChange={(event) => {
                  setRecursive(event.target.checked);
                  setSaved(false);
                }}
              />
              <span>
                {t('folderCard.advanced.recursive')}
                {folder.mode === 'auto' && (
                  <span className="mt-1 block text-xs text-primary-500">
                    {t('folderCard.advanced.recursiveAutoDisabled')}
                  </span>
                )}
              </span>
            </label>

            <div className="grid gap-3 sm:grid-cols-2">
              <label className="space-y-1 text-xs text-primary-400">
                <span>{t('folderCard.advanced.maxDepth')}</span>
                <select
                  className="input"
                  value={maxDepth}
                  disabled={!recursive || folder.mode === 'auto'}
                  onChange={(event) => {
                    setMaxDepth(event.target.value);
                    setSaved(false);
                  }}
                >
                  {[1, 2, 3, 5, 10, 20, 64].map((depth) => (
                    <option key={depth} value={depth}>
                      {depth}
                    </option>
                  ))}
                  <option value="unlimited">{t('folderCard.advanced.unlimited')}</option>
                </select>
              </label>
              <label className="space-y-1 text-xs text-primary-400">
                <span>{t('folderCard.advanced.destinationMode')}</span>
                <select
                  className="input"
                  value={destinationMode}
                  disabled={!recursive || folder.mode === 'auto'}
                  onChange={(event) => {
                    setDestinationMode(event.target.value as DestinationMode);
                    setSaved(false);
                  }}
                >
                  <option value="flatten">{t('folderCard.advanced.flatten')}</option>
                  <option value="preserve">{t('folderCard.advanced.preserve')}</option>
                </select>
              </label>
            </div>
          </section>

          <section className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1 text-xs text-primary-400">
              <span>{t('folderCard.advanced.excludeFolders')}</span>
              <textarea
                className="input min-h-24 font-mono text-xs"
                value={excludeFolders}
                onChange={(event) => {
                  setExcludeFolders(event.target.value);
                  setSaved(false);
                }}
                placeholder={'private\nexports/cache'}
              />
            </label>
            <label className="space-y-1 text-xs text-primary-400">
              <span>{t('folderCard.advanced.excludePatterns')}</span>
              <textarea
                className="input min-h-24 font-mono text-xs"
                value={excludePatterns}
                onChange={(event) => {
                  setExcludePatterns(event.target.value);
                  setSaved(false);
                }}
                placeholder={'*.tmp\n*.part'}
              />
            </label>
          </section>

          <div className="flex items-center justify-end gap-3">
            {saved && (
              <span className="text-xs text-accent-400">{t('folderCard.advanced.saved')}</span>
            )}
            <button
              type="button"
              className="btn-secondary"
              disabled={saving}
              onClick={() => void save()}
            >
              {t('common.save')}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}

function normalizedLines(value: string): string[] {
  return [
    ...new Set(
      value
        .split(/\r?\n/)
        .map((line) => line.trim())
        .filter(Boolean),
    ),
  ];
}

function depthText(depth: MaxDepth): string {
  if (depth === 'unlimited') return depth;
  if (typeof depth === 'number') return String(depth);
  return String(depth.level);
}
