import { M_PER_YD } from '../physics/golfCore';
import type { ShotRecord } from '../game/shot';

/** 마지막으로 착지한 샷의 결과 카드. 리셋해도 마지막 결과는 남는다. */
export function ResultCard({ shot }: { shot: ShotRecord | null }) {
  if (!shot) {
    return (
      <section className="result" aria-label="샷 결과">
        <p className="muted">아직 샷이 없습니다. 공 옆에서 샷을 해 보세요.</p>
      </section>
    );
  }
  const s = shot.stats;
  const lat = shot.lateralYd;
  // 부호를 붙여서 보여준다: + = 목표선 오른쪽, − = 왼쪽
  const latText = `${lat >= 0 ? '+' : '−'}${Math.abs(lat).toFixed(1)} yd`;
  return (
    <section className="result" aria-label="샷 결과">
      <div className="result-head">
        <strong>샷 {shot.id} 결과</strong>
      </div>
      <dl className="result-grid">
        <div>
          <dt>캐리</dt>
          <dd>
            {s.carryYd.toFixed(1)} yd <small>({s.carryM.toFixed(1)} m)</small>
          </dd>
        </div>
        <div>
          <dt>최고 높이</dt>
          <dd>
            {s.apexM.toFixed(1)} m <small>({(s.apexM / M_PER_YD).toFixed(1)} yd)</small>
          </dd>
        </div>
        <div>
          <dt>착지각</dt>
          <dd>{s.landDeg.toFixed(1)}°</dd>
        </div>
        <div>
          <dt>좌우 편차</dt>
          <dd>
            {latText} <small>목표선 기준</small>
          </dd>
        </div>
        <div>
          <dt>착지 속력</dt>
          <dd>{s.landMph.toFixed(0)} mph</dd>
        </div>
        <div>
          <dt>총거리</dt>
          <dd>
            {s.totalReliable ? (
              <>
                {s.totalYd.toFixed(0)} yd <small>참고: TrackMan 기본 표면, ±8 yd</small>
              </>
            ) : (
              <small>드라이버 발사 범위 밖이라 표시하지 않음</small>
            )}
          </dd>
        </div>
        <div>
          <dt>비행 시간</dt>
          <dd>{s.timeS.toFixed(2)} s</dd>
        </div>
      </dl>
    </section>
  );
}
