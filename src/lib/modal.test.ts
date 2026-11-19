import { describe, expect, it } from 'vitest';
import { modal, modalClosed, modalOpened } from './ui/modal.svelte';

describe('modal counter', () => {
  it('two opens and two closes return to 0', () => {
    modalOpened();
    modalOpened();
    expect(modal.open).toBe(2);
    modalClosed();
    modalClosed();
    expect(modal.open).toBe(0);
  });

  it('never goes negative', () => {
    modalClosed();
    expect(modal.open).toBe(0);
  });
});
