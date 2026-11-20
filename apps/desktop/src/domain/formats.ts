/** Formatos de exibição aceitos pela engine (`format` de expressões e colunas de tabela). */
export const FORMATS = [
  { value: '', label: '(nenhum)' },
  { value: 'currency:BRL', label: 'Moeda R$' },
  { value: 'currency:USD', label: 'Moeda US$' },
  { value: 'number:2', label: 'Número 2 casas' },
  { value: 'number:0', label: 'Número inteiro' },
  { value: 'percent:1', label: 'Percentual' },
  { value: 'date:%d/%m/%Y', label: 'Data dd/mm/aaaa' },
  { value: 'date:%d/%m/%Y %H:%M', label: 'Data e hora' },
  { value: 'upper', label: 'MAIÚSCULAS' },
];
