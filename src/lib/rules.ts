import type { Action, Condition, Rule } from '../ipc/types';

export const CONDITION_TYPES: Condition['type'][] = [
  'extension',
  'nameContains',
  'nameStartsWith',
  'nameEndsWith',
  'regex',
  'sizeGreaterThan',
  'sizeLessThan',
  'olderThan',
  'newerThan',
];

const NUMERIC_CONDITIONS = new Set<Condition['type']>([
  'sizeGreaterThan',
  'sizeLessThan',
  'olderThan',
  'newerThan',
]);

export function parseConditionValue(type: Condition['type'], raw: string): string[] {
  if (type === 'regex' || NUMERIC_CONDITIONS.has(type)) {
    const value = raw.trim();
    return value ? [value] : [];
  }
  return [
    ...new Set(
      raw
        .split(',')
        .map((value) => value.trim())
        .filter(Boolean),
    ),
  ];
}

export function conditionValueText(condition: Condition): string {
  return condition.type === 'regex' || NUMERIC_CONDITIONS.has(condition.type)
    ? (condition.value[0] ?? '')
    : condition.value.join(', ');
}

export function isValidCondition(condition: Condition): boolean {
  if (condition.value.length === 0 || condition.value.some((value) => !value.trim())) return false;
  if (NUMERIC_CONDITIONS.has(condition.type)) {
    const number = Number(condition.value[0]);
    return Number.isFinite(number) && number >= 0;
  }
  if (condition.type === 'regex') {
    try {
      new RegExp(condition.value[0] ?? '');
      return true;
    } catch {
      return false;
    }
  }
  return true;
}

export function isAbsoluteDestination(value: string): boolean {
  const withoutTokens = value.replace(/\{[a-zA-Z]+\}/g, 'token');
  return (
    withoutTokens.startsWith('/') ||
    /^[a-zA-Z]:[\\/]/.test(withoutTokens) ||
    /^\\\\[^\\]+\\[^\\]+/.test(withoutTokens)
  );
}

export function isManagedDestination(value: string): boolean {
  const normalized = value.trim().replaceAll('\\', '/');
  if (!normalized || normalized.includes('\0') || isAbsoluteDestination(normalized)) return false;
  const components = normalized.split('/');
  const root = components[0];
  if (
    !root ||
    components.some((component) => !component || component === '.' || component === '..') ||
    root.includes('{') ||
    root.includes('}')
  ) {
    return false;
  }
  return true;
}

export function isValidDestination(value: string): boolean {
  return isAbsoluteDestination(value) || isManagedDestination(value);
}

export function suggestManagedDestination(ruleName: string): string {
  const suggestion = ruleName
    .replace(/\{[^}]*\}/g, ' ')
    .replace(/[\0/\\]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/^\.+|\.+$/g, '')
    .trim();
  return suggestion === '.' || suggestion === '..' ? '' : suggestion;
}

export function isDangerousAction(action: Action): boolean {
  return action.type === 'delete' || action.type === 'runCommand';
}

export function isValidRule(rule: Pick<Rule, 'name' | 'conditions' | 'action'>): boolean {
  if (!rule.name.trim() || rule.name.trim().length > 100) return false;
  if (rule.conditions.length === 0 || !rule.conditions.every(isValidCondition)) return false;
  if (rule.action.type === 'move') {
    return Boolean(rule.action.destination.trim()) && isValidDestination(rule.action.destination);
  }
  if (rule.action.type === 'runCommand') {
    const timeout = rule.action.timeoutMs ?? 30_000;
    return Boolean(rule.action.command.trim()) && timeout >= 1_000 && timeout <= 600_000;
  }
  return true;
}
