import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WheelConfig } from '../types';

const api = vi.hoisted(() => ({
  getWheelConfig: vi.fn(),
  setWheelConfig: vi.fn(),
  resetWheelProfile: vi.fn(),
  resetWheelConfig: vi.fn(),
}));
vi.mock('../ipc', () => ({
  api,
  errorOf: (e: unknown) => (e && typeof e === 'object' && 'code' in e ? e : { code: 'failed', message: String(e) }),
}));

const { WheelConfigStore, SAVE_DELAY_MS } = await import('./wheelConfig.svelte');
const { toast } = await import('../ui/toaster.svelte');

const config = (version: number): WheelConfig => ({ version, presets: [], groups: [], profiles: [] });

describe('wheel config store', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
    toast.items = [];
  });
  afterEach(() => vi.useRealTimers());

  it('loads, and saves once per pause with the latest value', async () => {
    api.getWheelConfig.mockResolvedValue(config(1));
    api.setWheelConfig.mockImplementation(async (c: WheelConfig) => ({ ...c, version: 9 }));
    const store = new WheelConfigStore();
    await store.load();
    expect(store.value?.version).toBe(1);

    store.save(config(2));
    store.save(config(3));
    expect(store.value?.version).toBe(3); // optimistic
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS - 1);
    expect(api.setWheelConfig).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(api.setWheelConfig).toHaveBeenCalledTimes(1);
    expect(api.setWheelConfig).toHaveBeenCalledWith(config(3));
    expect(store.value?.version).toBe(9); // the backend's normalized copy
  });

  it('rolls back and shows an error toast when the backend rejects the config', async () => {
    api.getWheelConfig.mockResolvedValue(config(1));
    api.setWheelConfig.mockRejectedValue({ code: 'invalid', message: 'profile p front: 9 slots (max 8)' });
    const store = new WheelConfigStore();
    await store.load();
    store.save(config(2));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(store.value?.version).toBe(1);
    expect(toast.items).toHaveLength(1);
    expect(toast.items[0].kind).toBe('error');
  });

  it('keeps newer edits when an older save fails', async () => {
    api.getWheelConfig.mockResolvedValue(config(1));
    let reject!: (e: unknown) => void;
    api.setWheelConfig.mockImplementationOnce(() => new Promise((_, r) => (reject = r)));
    api.setWheelConfig.mockImplementationOnce(async (c: WheelConfig) => c);
    const store = new WheelConfigStore();
    await store.load();
    store.save(config(2));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    store.save(config(3));
    reject({ code: 'failed', message: 'x' });
    await vi.advanceTimersByTimeAsync(0);
    expect(store.value?.version).toBe(3);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(api.setWheelConfig).toHaveBeenLastCalledWith(config(3));
    expect(store.value?.version).toBe(3);
  });

  it('sends pending edits before a reset and adopts the reset result', async () => {
    api.getWheelConfig.mockResolvedValue(config(1));
    api.setWheelConfig.mockImplementation(async (c: WheelConfig) => c);
    api.resetWheelProfile.mockResolvedValue(config(5));
    const store = new WheelConfigStore();
    await store.load();
    store.save(config(2));
    const done = store.resetProfile('images');
    await vi.advanceTimersByTimeAsync(0);
    expect(await done).toBe(true);
    expect(api.setWheelConfig).toHaveBeenCalledWith(config(2));
    expect(api.resetWheelProfile).toHaveBeenCalledWith('images');
    expect(store.value?.version).toBe(5);
  });

  it('marks a failed first load', async () => {
    api.getWheelConfig.mockRejectedValue(new Error('no backend'));
    const store = new WheelConfigStore();
    await store.load();
    expect(store.value).toBeNull();
    expect(store.failed).toBe(true);
  });
});
