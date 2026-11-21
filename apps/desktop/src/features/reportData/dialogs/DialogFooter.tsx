/** Rodapé padrão dos diálogos: Ok grava e fecha, Cancelar só fecha. */
export function DialogFooter({ onOk, onCancel, error, okLabel = 'Ok' }: { onOk: () => void; onCancel: () => void; error?: string; okLabel?: string }) {
  return (
    <>
      {error && <span className="err small">{error}</span>}
      <span className="spacer" />
      <button className="primary" disabled={!!error} onClick={onOk}>{okLabel}</button>
      <button onClick={onCancel}>Cancelar</button>
    </>
  );
}
