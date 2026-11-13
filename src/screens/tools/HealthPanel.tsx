import { useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { gsap, reduced, useGSAP } from "../../lib/motion";
import { api } from "../../lib/ipc";
import { serial } from "../../lib/format";
import { keys, useSnapshot } from "../../app/queries";
import { useUi } from "../../app/store";
import { Detent, EmptyState, Sheet } from "../../ui/primitives";
import { Icon } from "../../ui/Icon";
import "./tools.css";

const CX = 130;
const CY = 132;
const angle = (score: number) => -90 + Math.max(0, Math.min(100, score)) * 1.8;
const polar = (score: number, r: number) => {
  const a = ((angle(score) - 90) * Math.PI) / 180;
  return [CX + r * Math.cos(a), CY + r * Math.sin(a)] as const;
};
const arc = (from: number, to: number, r: number) => {
  const [x1, y1] = polar(from, r);
  const [x2, y2] = polar(to, r);
  return `M${x1.toFixed(1)} ${y1.toFixed(1)} A${r} ${r} 0 0 1 ${x2.toFixed(1)} ${y2.toFixed(1)}`;
};

function Gauge({ score }: { score: number }) {
  const root = useRef<SVGSVGElement>(null);
  useGSAP(
    () => {
      gsap.fromTo(".gauge__needle", { rotation: angle(0), svgOrigin: `${CX} ${CY}` }, { rotation: angle(score), duration: reduced() ? 0 : 1.1, ease: "settle", delay: reduced() ? 0 : 0.25 });
    },
    { dependencies: [score], scope: root },
  );
  return (
    <svg ref={root} className="gauge" viewBox="0 0 260 150" role="img" aria-label={`Health score ${score} out of 100`}>
      <path className="gauge__zone is-bad" d={arc(0, 39, 108)} />
      <path className="gauge__zone is-warn" d={arc(41, 69, 108)} />
      <path className="gauge__zone is-good" d={arc(71, 100, 108)} />
      {Array.from({ length: 21 }, (_, i) => {
        const v = i * 5;
        const major = v % 20 === 0;
        const [x1, y1] = polar(v, 101);
        const [x2, y2] = polar(v, major ? 86 : 93);
        return <line key={v} x1={x1} y1={y1} x2={x2} y2={y2} className={major ? "gauge__tick is-major" : "gauge__tick"} />;
      })}
      {[0, 20, 40, 60, 80, 100].map((v) => {
        const [x, y] = polar(v, 72);
        return (
          <text key={v} x={x} y={y + 4} className="gauge__num" textAnchor="middle">
            {v}
          </text>
        );
      })}
      <path className="gauge__needle" d={`M${CX - 2.6} ${CY + 14} L${CX - 0.7} ${CY - 98} h1.4 L${CX + 2.6} ${CY + 14} z`} />
      <circle cx={CX} cy={CY} r="9" className="gauge__hub" />
      <circle cx={CX} cy={CY} r="2.5" className="gauge__pin" />
    </svg>
  );
}

export function HealthPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { data: report, isLoading } = useQuery({ queryKey: keys.health, queryFn: api.health, enabled: open, staleTime: 0 });
  const { data: snap } = useSnapshot();
  const select = useUi((s) => s.select);
  const setFilter = useUi((s) => s.setFilter);

  return (
    <Sheet open={open} onClose={onClose} title="Password health" width={680}>
      {isLoading || !report ? (
        <Detent size={24} />
      ) : report.totalPasswords === 0 ? (
        <EmptyState icon="gauge" title="No passwords to check yet">
          Health looks at every password field for weak, reused and old passwords. Nothing leaves this computer.
        </EmptyState>
      ) : (
        <div className="health">
          <div className="health__summary">
            <div className="health__score">
              <Gauge score={report.score} />
              <p className="health__read">
                <span className="health__num plate-type">{report.score}</span>
                <span className="health__of">out of 100</span>
              </p>
            </div>
            <dl className="health__counts">
              <div className={report.weak ? "is-bad" : ""}>
                <dt>Weak</dt>
                <dd className="code">{report.weak}</dd>
              </div>
              <div className={report.reused ? "is-bad" : ""}>
                <dt>Reused</dt>
                <dd className="code">{report.reused}</dd>
              </div>
              <div className={report.old ? "is-warn" : ""}>
                <dt>Older than {snap?.settings.passwordMaxAgeDays ?? 365} days</dt>
                <dd className="code">{report.old}</dd>
              </div>
              <div>
                <dt>Checked</dt>
                <dd className="code">{report.totalPasswords}</dd>
              </div>
            </dl>
          </div>
          {report.issues.length === 0 ? (
            <p className="health__ok">
              <Icon name="check" /> Every password is strong, unique and recent.
            </p>
          ) : (
            <ul className="health__list">
              {report.issues.map((i) => {
                const item = snap?.items.find((x) => x.id === i.itemId);
                return (
                  <li key={`${i.itemId}-${i.fieldId}`}>
                    <button
                      onClick={() => {
                        setFilter({ kind: "all" });
                        select(i.itemId);
                        onClose();
                      }}
                    >
                      <span className="code health__serial">{item ? serial(item.serial) : ""}</span>
                      <span className="health__title">
                        {item?.title ?? "Item"} <span className="health__field">{i.fieldName}</span>
                      </span>
                      <span className="health__tags">
                        {i.weak && <span className="is-bad">Weak</span>}
                        {i.reusedWith > 0 && <span className="is-bad">Used in {i.reusedWith + 1} items</span>}
                        {i.old && <span className="is-warn">{Math.round(i.ageDays / 30)} months old</span>}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      )}
    </Sheet>
  );
}
