import { useEffect, useState } from 'react';

import type { CoreInfo, ShotParams } from '../physics/golfCore';
import { PARAM_FIELDS, PRESETS, type ParamField, type PresetId } from '../physics/presets';
import { ClubTable } from './ClubTable';

interface ShotPanelProps {
  params: ShotParams;
  onField: (key: keyof ShotParams, value: number) => void;
  onPreset: (id: PresetId) => void;
  /** 코어 상태(SIMD 여부)와 클럽별 캐리: 클럽표용 */
  core: CoreInfo | null;
  clubCarries: Float64Array | null;
  onClub: (id: string) => void;
  onShoot: () => void;
  onReset: () => void;
  canShoot: boolean;
  hint: string;
  open: boolean;
  onToggle: () => void;
}

/** 샷 조작판. 프리셋, 다섯 가지 입력(슬라이더 + 숫자), 샷과 다시 놓기 버튼. */
export function ShotPanel({ params, onField, onPreset, core, clubCarries, onClub, onShoot, onReset, canShoot, hint, open, onToggle }: ShotPanelProps) {
  return (
    <section className={`panel${open ? '' : ' is-closed'}`} aria-label="샷 조작판">
      <button type="button" className="panel-toggle" aria-expanded={open} onClick={onToggle}>
        조작판 {open ? '접기' : '펼치기'}
      </button>
      {open && (
        <div className="panel-body">
          <div className="presets" role="group" aria-label="프리셋">
            {PRESETS.map((p) => (
              <button key={p.id} type="button" className="chip" onClick={() => onPreset(p.id)}>
                {p.label}
              </button>
            ))}
          </div>
          {PARAM_FIELDS.map((field) => (
            <ParamRow key={field.key} field={field} value={params[field.key]} onChange={(v) => onField(field.key, v)} />
          ))}
          <div className="actions">
            <button type="button" className="primary" onClick={onShoot} disabled={!canShoot}>
              샷
            </button>
            <button type="button" className="ghost" onClick={onReset}>
              다시 놓기
            </button>
          </div>
          <p className="hint" aria-live="polite">
            {hint}
          </p>
          <ClubTable carries={clubCarries} core={core} onPick={onClub} />
        </div>
      )}
    </section>
  );
}

/** 입력 한 줄. 숫자 칸은 입력 중에는 로컬 문자열을 쓰고, 포커스가 떠나거나 Enter 때 값을 반영한다. */
function ParamRow({ field, value, onChange }: { field: ParamField; value: number; onChange: (v: number) => void }) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  const commit = () => {
    const v = Number(draft);
    if (draft.trim() !== '' && Number.isFinite(v)) onChange(v);
    else setDraft(String(value));
  };

  const id = `param-${field.key}`;
  return (
    <div className="param">
      <div className="param-head">
        <label htmlFor={id}>{field.label}</label>
        {field.hint && <small>{field.hint}</small>}
      </div>
      <div className="param-ctrl">
        <input
          id={id}
          type="range"
          min={field.min}
          max={field.max}
          step={field.step}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
        />
        <input
          className="num"
          type="number"
          inputMode="decimal"
          aria-label={`${field.label} 숫자 입력`}
          min={field.min}
          max={field.max}
          step={field.step}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
          }}
        />
        <span className="unit">{field.unit}</span>
      </div>
    </div>
  );
}
