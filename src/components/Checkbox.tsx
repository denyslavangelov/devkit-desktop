import { Check } from "lucide-react";

type Props = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
  id?: string;
};

export function Checkbox({
  checked,
  onChange,
  label,
  description,
  disabled = false,
  id,
}: Props) {
  return (
    <label className={`ui-checkbox ${checked ? "checked" : ""} ${disabled ? "disabled" : ""}`}>
      <input
        id={id}
        type="checkbox"
        className="ui-checkbox-input"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="ui-checkbox-box" aria-hidden>
        <Check size={11} strokeWidth={3} className="ui-checkbox-icon" />
      </span>
      <span className="ui-checkbox-copy">
        <span className="ui-checkbox-label">{label}</span>
        {description && <span className="ui-checkbox-desc">{description}</span>}
      </span>
    </label>
  );
}
