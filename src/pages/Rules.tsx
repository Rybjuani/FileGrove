import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, Power } from 'lucide-react';
import { useAppStore } from '../stores/useAppStore';
import type { Rule, Condition, Action } from '../ipc/types';

export function Rules() {
  const { t } = useTranslation();
  const { rules, loadRules, toggleRule, deleteRule } = useAppStore();
  const [showEditor, setShowEditor] = useState(false);

  useEffect(() => {
    loadRules();
  }, [loadRules]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-primary-50">{t('rules.title')}</h1>
          <p className="mt-1 text-sm text-primary-400">{t('rules.subtitle')}</p>
        </div>
        <button className="btn-primary" onClick={() => setShowEditor(true)}>
          <Plus size={16} />
          {t('rules.newRule')}
        </button>
      </div>

      {rules.length === 0 ? (
        <div className="card py-12 text-center">
          <p className="text-sm text-primary-400">{t('rules.noCustomRules')}</p>
          <p className="mt-2 text-xs text-primary-500">{t('rules.createFirst')}</p>
        </div>
      ) : (
        <div className="space-y-2">
          {rules.map((rule) => (
            <RuleRow
              key={rule.id}
              rule={rule}
              onToggle={() => void toggleRule(rule.id)}
              onDelete={() => void deleteRule(rule.id)}
            />
          ))}
        </div>
      )}

      {showEditor && <RuleEditor onClose={() => setShowEditor(false)} />}
    </div>
  );
}

function RuleRow({
  rule,
  onToggle,
  onDelete,
}: {
  rule: Rule;
  onToggle: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="card flex items-center justify-between">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-primary-100">{rule.name}</span>
          {rule.isQuickRule && (
            <span className="badge bg-accent-500/15 text-accent-400">quick</span>
          )}
        </div>
        <p className="mt-1 truncate text-xs text-primary-500">
          {rule.conditions.length} {t('rules.noConditions').toLowerCase()}
          {' · '}
          {rule.action.type}
        </p>
      </div>
      <div className="flex items-center gap-1">
        <button
          className={`btn-ghost ${rule.enabled ? 'text-accent-400' : 'text-primary-500'}`}
          onClick={onToggle}
          title={rule.enabled ? t('rules.disable') : t('rules.enable')}
        >
          <Power size={14} />
        </button>
        <button className="btn-ghost text-red-400" onClick={onDelete} title={t('common.delete')}>
          <Trash2 size={14} />
        </button>
      </div>
    </div>
  );
}

function RuleEditor({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const { createRule } = useAppStore();
  const [name, setName] = useState('');
  const [extensions, setExtensions] = useState('');
  const [destination, setDestination] = useState('');

  const handleSave = async () => {
    const extensionValues = extensions
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
      .map((ext) => (ext.startsWith('.') ? ext : `.${ext}`));
    const conds: Condition[] = extensionValues.length
      ? [{ type: 'extension', value: extensionValues }]
      : [];

    const action: Action = { type: 'move', destination: destination || 'Organized' };
    const rule: Rule = {
      id: crypto.randomUUID(),
      name: name || 'Untitled rule',
      isQuickRule: false,
      enabled: true,
      order: 0,
      icon: 'File',
      conditions: conds,
      action,
      createdAt: new Date().toISOString(),
    };
    await createRule(rule);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="card w-full max-w-lg space-y-4">
        <h2 className="text-lg font-semibold">{t('rules.createRule')}</h2>
        <div>
          <label className="block text-xs font-medium text-primary-400">
            {t('rules.ruleName')}
          </label>
          <input
            className="input mt-1"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Photos"
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-primary-400">
            {t('rules.extensions')}
          </label>
          <input
            className="input mt-1"
            value={extensions}
            onChange={(e) => setExtensions(e.target.value)}
            placeholder=".jpg, .png, .gif"
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-primary-400">
            {t('rules.destination')}
          </label>
          <input
            className="input mt-1"
            value={destination}
            onChange={(e) => setDestination(e.target.value)}
            placeholder="Photos"
          />
        </div>
        <div className="flex justify-end gap-2 pt-4">
          <button className="btn-ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn-primary" onClick={() => void handleSave()}>
            {t('rules.createRuleBtn')}
          </button>
        </div>
      </div>
    </div>
  );
}
