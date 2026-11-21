import { resolveApiBaseUrl } from '../baseUrl';

describe('resolveApiBaseUrl', () => {
  it('prefers the env override and trims trailing slashes', () => {
    expect(resolveApiBaseUrl('http://192.168.1.50:8080/', 'ios', undefined)).toBe(
      'http://192.168.1.50:8080',
    );
  });

  it('uses the page origin on web when no override is set', () => {
    expect(resolveApiBaseUrl(undefined, 'web', 'http://192.168.1.50:8080')).toBe(
      'http://192.168.1.50:8080',
    );
  });

  it('ignores a blank override', () => {
    expect(resolveApiBaseUrl('  ', 'web', 'http://host:8080')).toBe('http://host:8080');
  });

  it('falls back to localhost on native without an override', () => {
    expect(resolveApiBaseUrl(undefined, 'android', undefined)).toBe('http://localhost:8080');
  });
});
