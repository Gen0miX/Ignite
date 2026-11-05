import { defaultRow, fromRules, toRules, type EditorModel } from './ruleRows';

describe('rule rows', () => {
  it('converts rows to rules with units', () => {
    const model: EditorModel = {
      match: 'all',
      rows: [
        { field: 'tag', op: 'in', value: 'souls-like', value2: '' },
        { field: 'playtime_minutes', op: 'gt', value: '20', value2: '' },
        { field: 'avg_session_minutes', op: 'between', value: '30', value2: '60' },
        { field: 'installed', op: 'eq', value: 'false', value2: '' },
        { field: 'last_played', op: 'older_than_days', value: '365', value2: '' },
      ],
    };
    expect(JSON.parse(toRules(model)!)).toEqual({
      all: [
        { field: 'tag', op: 'in', value: ['souls-like'] },
        { field: 'playtime_minutes', op: 'gt', value: 1200 },
        { field: 'avg_session_minutes', op: 'between', value: [30, 60] },
        { field: 'installed', op: 'eq', value: false },
        { field: 'last_played', op: 'older_than_days', value: 365 },
      ],
    });
  });

  it('round-trips and rejects incomplete rows', () => {
    const model: EditorModel = {
      match: 'any',
      rows: [
        { field: 'source', op: 'in', value: 'gog', value2: '' },
        { field: 'playtime_minutes', op: 'between', value: '1,5', value2: '10' },
      ],
    };
    expect(fromRules(toRules(model))).toEqual({
      match: 'any',
      rows: [
        { field: 'source', op: 'in', value: 'gog', value2: '' },
        { field: 'playtime_minutes', op: 'between', value: '1.5', value2: '10' },
      ],
    });
    expect(toRules({ match: 'all', rows: [{ ...defaultRow('tag'), value: ' ' }] })).toBeNull();
    expect(
      toRules({ match: 'all', rows: [{ ...defaultRow('playtime_minutes'), value: 'abc' }] }),
    ).toBeNull();
  });

  it('refuses nested or negated rules (kept as advanced JSON)', () => {
    expect(fromRules('{"all":[{"not":{"field":"source","op":"in","value":["gog"]}}]}')).toBeNull();
    expect(fromRules('{"all":[{"any":[]}]}')).toBeNull();
    expect(fromRules('not json')).toBeNull();
    expect(fromRules(null)).toEqual({ match: 'all', rows: [] });
  });
});
