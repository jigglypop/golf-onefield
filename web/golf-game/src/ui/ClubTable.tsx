import type { CoreInfo } from '../physics/golfCore';
import { CLUBS } from '../physics/presets';

interface ClubTableProps {
  /** 클럽별 캐리(yd), CLUBS 와 같은 순서. 아직 계산 전이면 null */
  carries: Float64Array | null;
  /** 코어 상태 (SIMD 여부). 아직 로드 전이면 null */
  core: CoreInfo | null;
  onPick: (id: string) => void;
}

/**
 * 클럽별 캐리 표. 12클럽을 일괄 계산 한 번(batch_simd_opts)으로 구한다.
 * SIMD 가 없는 브라우저에서는 같은 계산이 스칼라 f32 로 돌아서 일괄 처리 속도 목표(샷당 1 µs)가 성립하지 않으므로 그 사실을 보여 준다.
 * 이 표의 캐리는 시뮬레이션 값이다 (입력인 발사 조건만 PGA 투어 평균 실측).
 */
export function ClubTable({ carries, core, onPick }: ClubTableProps) {
  return (
    <details className="clubs">
      <summary>클럽별 캐리 (시뮬레이션)</summary>
      <ul className="club-list">
        {CLUBS.map((club, i) => (
          <li key={club.id}>
            <button type="button" className="club-row" onClick={() => onPick(club.id)}>
              <span>{club.label}</span>
              <span className="club-carry">{carries ? `${carries[i]!.toFixed(0)} yd` : '…'}</span>
            </button>
          </li>
        ))}
      </ul>
      {core && (
        <p className={core.simd ? 'core-note' : 'core-note core-warn'} role={core.simd ? undefined : 'alert'}>
          {core.simd
            ? '일괄 계산: wasm SIMD 4레인 (f32)'
            : '이 브라우저는 wasm SIMD 가 없어 일괄 계산이 스칼라 f32 로 돕니다. 샷당 1 µs 목표는 성립하지 않습니다 (샷 하나의 계산은 영향 없음).'}
        </p>
      )}
      <p className="core-note">발사 조건은 PGA 투어 평균(실측), 캐리는 엔진 계산값입니다. 누르면 입력에 적용됩니다.</p>
    </details>
  );
}
