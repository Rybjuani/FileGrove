import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Archive,
  ArrowDown,
  ArrowUp,
  Download,
  Edit3,
  FileText,
  Image,
  Music,
  Plus,
  Power,
  Search,
  Trash2,
  Video,
} from 'lucide-react';
import { RuleEditor } from '../components/RuleEditor';
import { ipc } from '../ipc';
import type { Action, Rule } from '../ipc/types';
import { useAppStore } from '../stores/useAppStore';

export function Rules() {
  const { t } = useTranslation();
  const { folders, rules, loadRules, toggleRule, deleteRule, reportError } = useAppStore();
  const [query, setQuery] = useState('');
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<Rule | undefined>();
  const [createAsNew, setCreateAsNew] = useState(false);
  const [reordering, setReordering] = useState(false);

  useEffect(() => {
    void loadRules();
  }, [loadRules]);

  const orderedRules = useMemo(
    () => [...rules].sort((a, b) => a.order - b.order || a.createdAt.localeCompare(b.createdAt)),
    [rules],
  );
  const filteredRules = orderedRules.filter((rule) =>
    rule.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
  );
  const nextOrder = Math.max(-1, ...rules.map((rule) => rule.order)) + 1;

  const openEditor = (rule?: Rule, asNew = false) => {
    setEditingRule(rule);
    setCreateAsNew(asNew);
    setEditorOpen(true);
  };

  const openTemplate = (template: QuickRuleTemplate) => {
    openEditor(
      {
        id: crypto.randomUUID(),
        name: t(`quickRules.${template.key}`),
        isQuickRule: false,
        enabled: true,
        order: nextOrder,
        icon: template.iconName,
        conditions: [{ type: 'extension', value: [...template.extensions] }],
        action: { type: 'move', destination: '' },
        createdAt: new Date().toISOString(),
      },
      true,
    );
  };

  const moveRule = async (ruleId: string, direction: -1 | 1) => {
    const index = orderedRules.findIndex((rule) => rule.id === ruleId);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= orderedRules.length) return;

    const next = [...orderedRules];
    const [moved] = next.splice(index, 1);
    next.splice(target, 0, moved);
    setReordering(true);
    const response = await ipc.rules.reorder(next.map((rule) => rule.id));
    if (!response.success && response.error) reportError(response.error);
    await loadRules();
    setReordering(false);
  };

  const confirmDelete = async (rule: Rule) => {
    if (window.confirm(t('rules.deleteConfirm', { name: rule.name }))) {
      await deleteRule(rule.id);
    }
  };

  const toggleWithConfirmation = async (rule: Rule) => {
    if (!rule.enabled && isDangerousAction(rule.action)) {
      const activeFolders = folders.filter(
        (folder) =>
          folder.ruleIds.includes(rule.id) &&
          (folder.mode === 'auto' ||
            (folder.mode === 'scheduled' && Boolean(folder.schedule?.enabled))),
      );
      if (
        activeFolders.length > 0 &&
        !window.confirm(
          t('rules.enableDangerousConfirm', {
            name: rule.name,
            count: activeFolders.length,
            folders: activeFolders.map((folder) => folder.name).join(', '),
          }),
        )
      ) {
        return;
      }
    }
    await toggleRule(rule.id);
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold text-primary-50">{t('rules.title')}</h1>
          <p className="mt-1 text-sm text-primary-400">{t('rules.subtitle')}</p>
        </div>
        <button className="btn-primary" onClick={() => openEditor()}>
          <Plus size={16} />
          {t('rules.newRule')}
        </button>
      </div>

      <section className="space-y-3">
        <div>
          <h2 className="text-sm font-semibold text-primary-200">{t('rules.templatesTitle')}</h2>
          <p className="mt-1 text-xs text-primary-500">{t('rules.templatesDesc')}</p>
        </div>
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
          {QUICK_RULE_TEMPLATES.map((template) => {
            const Icon = template.icon;
            return (
              <button
                key={template.key}
                className="card flex items-center gap-3 p-3 text-left transition-colors hover:border-primary-700 hover:bg-primary-800/50"
                onClick={() => openTemplate(template)}
              >
                <span className="rounded-md bg-accent-500/10 p-2 text-accent-400">
                  <Icon size={18} />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium text-primary-200">
                    {t(`quickRules.${template.key}`)}
                  </span>
                  <span className="block truncate text-[11px] text-primary-500">
                    {template.extensions.slice(0, 5).join(', ')}
                  </span>
                </span>
                <Plus size={14} className="text-primary-500" />
              </button>
            );
          })}
        </div>
      </section>

      {rules.length > 0 && (
        <div className="relative max-w-md">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-primary-500" />
          <input
            className="input pl-9"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('rules.searchPlaceholder')}
          />
        </div>
      )}

      {rules.length === 0 ? (
        <div className="card py-12 text-center">
          <p className="text-sm text-primary-400">{t('rules.noCustomRules')}</p>
          <p className="mt-2 text-xs text-primary-500">{t('rules.createFirst')}</p>
        </div>
      ) : filteredRules.length === 0 ? (
        <div className="card py-10 text-center text-sm text-primary-400">
          {t('rules.noMatches')}
        </div>
      ) : (
        <div className="space-y-2">
          {filteredRules.map((rule) => {
            const absoluteIndex = orderedRules.findIndex((candidate) => candidate.id === rule.id);
            return (
              <RuleRow
                key={rule.id}
                rule={rule}
                first={absoluteIndex === 0}
                last={absoluteIndex === orderedRules.length - 1}
                disabled={reordering}
                onMove={(direction) => void moveRule(rule.id, direction)}
                onEdit={() => openEditor(rule)}
                onToggle={() => void toggleWithConfirmation(rule)}
                onDelete={() => void confirmDelete(rule)}
              />
            );
          })}
        </div>
      )}

      {editorOpen && (
        <RuleEditor
          rule={editingRule}
          nextOrder={nextOrder}
          createAsNew={createAsNew}
          onClose={() => {
            setEditorOpen(false);
            setEditingRule(undefined);
            setCreateAsNew(false);
          }}
        />
      )}
    </div>
  );
}

const QUICK_RULE_TEMPLATES = [
  {
    key: 'images',
    icon: Image,
    iconName: 'Image',
    extensions: ['.jpg', '.jpeg', '.png', '.gif', '.bmp', '.webp', '.svg', '.ico', '.heic'],
  },
  {
    key: 'documents',
    icon: FileText,
    iconName: 'FileText',
    extensions: ['.pdf', '.doc', '.docx', '.xls', '.xlsx', '.ppt', '.pptx', '.txt', '.rtf', '.odt'],
  },
  {
    key: 'videos',
    icon: Video,
    iconName: 'Video',
    extensions: ['.mp4', '.mkv', '.avi', '.mov', '.wmv', '.flv', '.webm', '.m4v'],
  },
  {
    key: 'audio',
    icon: Music,
    iconName: 'Music',
    extensions: ['.mp3', '.wav', '.flac', '.aac', '.ogg', '.wma', '.m4a'],
  },
  {
    key: 'archives',
    icon: Archive,
    iconName: 'Archive',
    extensions: ['.zip', '.rar', '.7z', '.tar', '.gz', '.bz2'],
  },
  {
    key: 'installers',
    icon: Download,
    iconName: 'Download',
    extensions: ['.exe', '.msi', '.dmg', '.app', '.deb', '.rpm'],
  },
] as const;

type QuickRuleTemplate = (typeof QUICK_RULE_TEMPLATES)[number];

function RuleRow({
  rule,
  first,
  last,
  disabled,
  onMove,
  onEdit,
  onToggle,
  onDelete,
}: {
  rule: Rule;
  first: boolean;
  last: boolean;
  disabled: boolean;
  onMove: (direction: -1 | 1) => void;
  onEdit: () => void;
  onToggle: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className={`card flex items-center gap-3 ${rule.enabled ? '' : 'opacity-60'}`}>
      <div className="flex shrink-0 flex-col">
        <button
          className="rounded p-1 text-primary-500 hover:bg-primary-800 hover:text-primary-200 disabled:opacity-25"
          onClick={() => onMove(-1)}
          disabled={disabled || first}
          aria-label={t('rules.moveUp')}
        >
          <ArrowUp size={14} />
        </button>
        <button
          className="rounded p-1 text-primary-500 hover:bg-primary-800 hover:text-primary-200 disabled:opacity-25"
          onClick={() => onMove(1)}
          disabled={disabled || last}
          aria-label={t('rules.moveDown')}
        >
          <ArrowDown size={14} />
        </button>
      </div>
      <button className="min-w-0 flex-1 text-left" onClick={onEdit}>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium text-primary-100">{rule.name}</span>
          {rule.isQuickRule && <span className="badge-warning">{t('rules.quick')}</span>}
          <span className="badge bg-primary-800 text-primary-400">#{rule.order}</span>
          {isDangerousAction(rule.action) && (
            <span className="badge-danger">{t(`rules.action.${rule.action.type}`)}</span>
          )}
        </div>
        <p className="mt-1 truncate text-xs text-primary-500">
          {t('rules.conditionCount', { count: rule.conditions.length })}
          {' · '}
          {actionSummary(rule.action, t)}
        </p>
      </button>
      <div className="flex items-center gap-1">
        <button className="btn-ghost p-2" onClick={onEdit} title={t('common.edit')}>
          <Edit3 size={15} />
        </button>
        <button
          className={`btn-ghost p-2 ${rule.enabled ? 'text-accent-400' : 'text-primary-500'}`}
          onClick={onToggle}
          title={rule.enabled ? t('rules.disable') : t('rules.enable')}
        >
          <Power size={15} />
        </button>
        <button
          className="btn-ghost p-2 text-red-400"
          onClick={onDelete}
          title={t('common.delete')}
        >
          <Trash2 size={15} />
        </button>
      </div>
    </div>
  );
}

function isDangerousAction(action: Action) {
  return action.type === 'delete' || action.type === 'runCommand';
}

function actionSummary(action: Action, t: ReturnType<typeof useTranslation>['t']) {
  if (action.type === 'move') return `${t('rules.action.move')} ${action.destination}`;
  if (action.type === 'runCommand') return t('rules.action.runCommand');
  return t(`rules.action.${action.type}`);
}
