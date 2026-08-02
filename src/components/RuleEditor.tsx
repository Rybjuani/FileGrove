import { useMemo, useState, type FormEvent } from 'react';
import { FolderOpen, Play, Plus, Trash2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ipc } from '../ipc';
import type { Action, CommandTestResult, Condition, Rule } from '../ipc/types';
import { readableIpcError } from '../lib/errors';
import {
  CONDITION_TYPES,
  conditionValueText,
  isAbsoluteDestination,
  isValidCondition,
  parseConditionValue,
} from '../lib/rules';
import { useAppStore } from '../stores/useAppStore';

interface Props {
  rule?: Rule;
  nextOrder: number;
  createAsNew?: boolean;
  onClose: () => void;
}

const DESTINATION_TOKENS = [
  '{year}',
  '{month}',
  '{day}',
  '{date}',
  '{createddate}',
  '{ext}',
  '{name}',
  '{folder}',
  '{exifdate}',
  '{exifyear}',
  '{camera}',
  '{lens}',
  '{iso}',
  '{dimensions}',
];

export function RuleEditor({ rule, nextOrder, createAsNew = false, onClose }: Props) {
  const { t } = useTranslation();
  const { createRule, updateRule } = useAppStore();
  const [name, setName] = useState(rule?.name ?? '');
  const [enabled, setEnabled] = useState(rule?.enabled ?? true);
  const [conditions, setConditions] = useState<Condition[]>(
    rule?.conditions.map((condition) => ({ ...condition, value: [...condition.value] })) ?? [
      { type: 'extension', value: [] },
    ],
  );
  const [action, setAction] = useState<Action>(
    rule?.action.type === 'runCommand'
      ? { ...rule.action, timeoutMs: rule.action.timeoutMs ?? 30_000 }
      : (rule?.action ?? { type: 'move', destination: '' }),
  );
  const [warningAccepted, setWarningAccepted] = useState(false);
  const [deleteAccepted, setDeleteAccepted] = useState(false);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [testRunning, setTestRunning] = useState(false);
  const [testResult, setTestResult] = useState<CommandTestResult | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  const title = rule && !createAsNew ? t('rules.editRule') : t('rules.createRule');
  const timeoutSeconds = action.type === 'runCommand' ? (action.timeoutMs ?? 30_000) / 1_000 : 30;
  const hasInvalidCondition = useMemo(
    () => conditions.some((condition) => !isValidCondition(condition)),
    [conditions],
  );

  const setActionType = (type: Action['type']) => {
    setTestResult(null);
    setTestError(null);
    if (type === 'move') setAction({ type, destination: '' });
    else if (type === 'runCommand') {
      setAction({ type, command: '', timeoutMs: 30_000, rerunAlways: false });
    } else setAction({ type });
  };

  const updateCondition = (index: number, next: Condition) => {
    setConditions((current) =>
      current.map((condition, conditionIndex) => (conditionIndex === index ? next : condition)),
    );
  };

  const chooseDestination = async () => {
    const response = await ipc.folders.selectDestination();
    if (response.success && response.data) {
      setAction({ type: 'move', destination: response.data });
    } else if (response.error !== 'USER_CANCELED_FOLDER_SELECT') {
      setFormError(readableIpcError(response.error, t));
    }
  };

  const testCommand = async () => {
    if (action.type !== 'runCommand' || !action.command.trim()) {
      setTestError(t('rules.validation.command'));
      return;
    }
    setTestRunning(true);
    setTestResult(null);
    setTestError(null);
    const response = await ipc.rules.testCommand(action.command, action.timeoutMs ?? 30_000);
    setTestRunning(false);
    if (response.success && response.data) setTestResult(response.data);
    else if (response.error !== 'USER_CANCELED_FILE_SELECT') {
      setTestError(readableIpcError(response.error, t));
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setFormError(null);
    if (!name.trim()) return setFormError(t('rules.validation.name'));
    if (conditions.length === 0) return setFormError(t('rules.validation.conditions'));
    if (hasInvalidCondition) return setFormError(t('rules.validation.invalidCondition'));
    if (action.type === 'move') {
      if (!action.destination.trim()) return setFormError(t('rules.validation.destination'));
      if (!isAbsoluteDestination(action.destination)) {
        return setFormError(t('rules.validation.destinationAbsolute'));
      }
    }
    if (action.type === 'runCommand') {
      if (!action.command.trim()) return setFormError(t('rules.validation.command'));
      const timeout = action.timeoutMs ?? 30_000;
      if (timeout < 1_000 || timeout > 600_000) {
        return setFormError(t('rules.validation.timeout'));
      }
      if (!warningAccepted) return setFormError(t('rules.validation.acceptWarning'));
    }
    if (action.type === 'delete' && !deleteAccepted) {
      return setFormError(t('rules.validation.acceptWarning'));
    }

    const nextRule: Rule = {
      id: rule && !createAsNew ? rule.id : crypto.randomUUID(),
      name: name.trim(),
      isQuickRule: rule && !createAsNew ? rule.isQuickRule : false,
      enabled,
      order: rule && !createAsNew ? rule.order : nextOrder,
      icon: rule?.icon ?? 'File',
      conditions,
      action,
      createdAt: rule && !createAsNew ? rule.createdAt : new Date().toISOString(),
    };
    setSaving(true);
    const response =
      rule && !createAsNew ? await updateRule(rule.id, nextRule) : await createRule(nextRule);
    setSaving(false);
    if (response.success) onClose();
    else setFormError(readableIpcError(response.error, t));
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <form
        className="card max-h-[92vh] w-full max-w-3xl overflow-hidden p-0"
        role="dialog"
        aria-modal="true"
        aria-labelledby="rule-editor-title"
        onSubmit={(event) => void submit(event)}
      >
        <header className="flex items-center justify-between border-b border-primary-800 px-5 py-4">
          <h2 id="rule-editor-title" className="text-lg font-semibold text-primary-50">
            {title}
          </h2>
          <button type="button" className="btn-ghost p-2" onClick={onClose}>
            <X size={18} />
          </button>
        </header>

        <div className="max-h-[72vh] space-y-6 overflow-y-auto p-5">
          <label className="block space-y-2 text-sm text-primary-300">
            <span>{t('rules.ruleName')}</span>
            <input
              className="input"
              value={name}
              maxLength={100}
              onChange={(event) => setName(event.target.value)}
              placeholder={t('rules.form.namePlaceholder')}
              autoFocus
            />
          </label>

          <section className="space-y-3">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h3 className="text-sm font-medium text-primary-200">
                  {t('rules.form.label.questionHeader')}
                </h3>
                <p className="mt-1 text-xs text-primary-500">{t('rules.form.label.allOfThese')}</p>
              </div>
              <button
                type="button"
                className="btn-secondary"
                onClick={() =>
                  setConditions((current) => [...current, { type: 'extension', value: [] }])
                }
              >
                <Plus size={14} /> {t('rules.addCondition')}
              </button>
            </div>

            {conditions.map((condition, index) => (
              <div
                key={`${index}-${condition.type}`}
                className="grid gap-2 rounded-md border border-primary-800 bg-primary-950/50 p-3 sm:grid-cols-[13rem_minmax(0,1fr)_auto]"
              >
                <select
                  className="input"
                  aria-label={t('rules.form.conditionRowLabel', { index: index + 1 })}
                  value={condition.type}
                  onChange={(event) =>
                    updateCondition(index, {
                      type: event.target.value as Condition['type'],
                      value: [],
                    } as Condition)
                  }
                >
                  {CONDITION_TYPES.map((type) => (
                    <option key={type} value={type}>
                      {t(`rules.form.conditionLabels.${type}`)}
                    </option>
                  ))}
                </select>
                <input
                  className="input"
                  value={conditionValueText(condition)}
                  onChange={(event) =>
                    updateCondition(index, {
                      type: condition.type,
                      value: parseConditionValue(condition.type, event.target.value),
                    } as Condition)
                  }
                  placeholder={t(`rules.form.placeholders.${condition.type}`)}
                />
                <button
                  type="button"
                  className="btn-ghost p-2 text-red-400"
                  onClick={() =>
                    setConditions((current) =>
                      current.filter((_, itemIndex) => itemIndex !== index),
                    )
                  }
                  aria-label={t('rules.form.removeCondition')}
                >
                  <Trash2 size={15} />
                </button>
              </div>
            ))}
          </section>

          <section className="space-y-3">
            <h3 className="text-sm font-medium text-primary-200">{t('rules.action.label')}</h3>
            <select
              className="input"
              value={action.type}
              onChange={(event) => setActionType(event.target.value as Action['type'])}
            >
              {(['move', 'trash', 'delete', 'runCommand'] as const).map((type) => (
                <option key={type} value={type}>
                  {t(`rules.action.${type}`)}
                </option>
              ))}
            </select>

            {action.type === 'move' && (
              <div className="space-y-3">
                <label className="block space-y-2 text-sm text-primary-300">
                  <span>{t('rules.destination')}</span>
                  <div className="flex gap-2">
                    <input
                      className="input min-w-0 flex-1 font-mono text-xs"
                      value={action.destination}
                      onChange={(event) =>
                        setAction({ type: 'move', destination: event.target.value })
                      }
                      placeholder={t('rules.destinationPlaceholder')}
                    />
                    <button type="button" className="btn-secondary" onClick={chooseDestination}>
                      <FolderOpen size={15} /> {t('rules.selectDestination')}
                    </button>
                  </div>
                </label>
                <div>
                  <p className="mb-2 text-xs text-primary-500">{t('rules.tokens.help')}</p>
                  <div className="flex flex-wrap gap-1">
                    {DESTINATION_TOKENS.map((token) => (
                      <button
                        key={token}
                        type="button"
                        className="rounded bg-primary-800 px-2 py-1 font-mono text-[11px] text-primary-300"
                        onClick={() =>
                          setAction({ type: 'move', destination: `${action.destination}${token}` })
                        }
                      >
                        {token}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {action.type === 'trash' && (
              <p className="rounded-md bg-primary-800/50 p-3 text-xs text-primary-400">
                {t('rules.action.trashHint')}
              </p>
            )}

            {action.type === 'delete' && (
              <div className="space-y-3 rounded-md bg-red-500/10 p-3 text-sm text-red-200">
                <p>{t('rules.action.deleteWarning')}</p>
                <label className="flex items-start gap-2 text-xs">
                  <input
                    type="checkbox"
                    checked={deleteAccepted}
                    onChange={(event) => setDeleteAccepted(event.target.checked)}
                  />
                  {t('rules.action.deleteConfirm')}
                </label>
              </div>
            )}

            {action.type === 'runCommand' && (
              <div className="space-y-4">
                <label className="block space-y-2 text-sm text-primary-300">
                  <span>{t('rules.action.command')}</span>
                  <textarea
                    className="input min-h-24 font-mono text-xs"
                    value={action.command}
                    onChange={(event) => setAction({ ...action, command: event.target.value })}
                    placeholder={t('rules.action.commandPlaceholder')}
                  />
                </label>
                <label className="block space-y-2 text-sm text-primary-300">
                  <span>{t('rules.action.timeout')}</span>
                  <input
                    className="input w-40"
                    type="number"
                    min={1}
                    max={600}
                    value={timeoutSeconds}
                    onChange={(event) =>
                      setAction({ ...action, timeoutMs: Number(event.target.value) * 1_000 })
                    }
                  />
                </label>
                <label className="flex items-start gap-2 text-xs text-primary-300">
                  <input
                    type="checkbox"
                    checked={action.rerunAlways ?? false}
                    onChange={(event) =>
                      setAction({ ...action, rerunAlways: event.target.checked })
                    }
                  />
                  <span>
                    {t('rules.action.rerunAlways')}
                    <span className="mt-1 block text-primary-500">
                      {t(
                        action.rerunAlways
                          ? 'rules.action.rerunAlwaysHint'
                          : 'rules.action.rerunOnceHint',
                      )}
                    </span>
                  </span>
                </label>
                <div className="rounded-md bg-yellow-500/10 p-3 text-sm text-yellow-100">
                  <p className="font-medium">{t('rules.action.securityWarning.title')}</p>
                  <p className="mt-1 text-xs text-yellow-200/80">
                    {t('rules.action.securityWarning.body')}
                  </p>
                  <label className="mt-3 flex items-start gap-2 text-xs">
                    <input
                      type="checkbox"
                      checked={warningAccepted}
                      onChange={(event) => setWarningAccepted(event.target.checked)}
                    />
                    {t('rules.action.securityWarning.accept')}
                  </label>
                </div>
                <button
                  type="button"
                  className="btn-secondary"
                  disabled={testRunning}
                  onClick={() => void testCommand()}
                >
                  <Play size={14} />
                  {t(testRunning ? 'rules.action.testRunning' : 'rules.action.testButton')}
                </button>
                <p className="text-xs text-primary-500">{t('rules.action.testHint')}</p>
                {testResult && (
                  <div className="rounded-md border border-primary-800 bg-primary-950 p-3 text-xs">
                    <p className="text-primary-300">
                      {t('rules.action.testResult', {
                        code: testResult.exitCode ?? '—',
                        duration: testResult.durationMs,
                      })}
                    </p>
                    {testResult.output && (
                      <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap text-primary-400">
                        {testResult.output}
                      </pre>
                    )}
                  </div>
                )}
                {testError && <p className="text-xs text-red-300">{testError}</p>}
              </div>
            )}
          </section>

          <label className="flex items-center gap-2 text-sm text-primary-300">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(event) => setEnabled(event.target.checked)}
            />
            {t('rules.enabledOnSave')}
          </label>

          {formError && (
            <p className="rounded-md bg-red-500/10 p-3 text-sm text-red-300">{formError}</p>
          )}
        </div>

        <footer className="flex justify-end gap-2 border-t border-primary-800 px-5 py-4">
          <button type="button" className="btn-secondary" onClick={onClose} disabled={saving}>
            {t('common.cancel')}
          </button>
          <button type="submit" className="btn-primary" disabled={saving}>
            {t(rule && !createAsNew ? 'rules.saveChanges' : 'rules.createRuleBtn')}
          </button>
        </footer>
      </form>
    </div>
  );
}
