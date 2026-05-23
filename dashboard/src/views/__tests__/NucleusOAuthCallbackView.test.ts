import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount } from '@vue/test-utils'
import NucleusOAuthCallbackView from '@/views/NucleusOAuthCallbackView.vue'

describe('NucleusOAuthCallbackView', () => {
  let originalLocation: Location
  let originalOpener: Window['opener']
  let postMessageSpy: ReturnType<typeof vi.fn>
  let closeSpy: ReturnType<typeof vi.fn>

  beforeEach(() => {
    originalLocation = window.location
    originalOpener = window.opener
    postMessageSpy = vi.fn()
    closeSpy = vi.fn()

    // jsdom's window.location is read-only; replace with a writable shim that
    // satisfies the type at the call sites we exercise.
    Object.defineProperty(window, 'location', {
      configurable: true,
      writable: true,
      value: {
        ...originalLocation,
        search: '?code=auth-code-xyz&state=csrf-token-abc',
        origin: 'http://localhost',
      },
    })
    window.close = closeSpy
  })

  afterEach(() => {
    Object.defineProperty(window, 'location', {
      configurable: true,
      writable: true,
      value: originalLocation,
    })
    Object.defineProperty(window, 'opener', {
      configurable: true,
      writable: true,
      value: originalOpener,
    })
  })

  it('postMessages code and state to opener and closes window', () => {
    Object.defineProperty(window, 'opener', {
      configurable: true,
      writable: true,
      value: { postMessage: postMessageSpy },
    })

    mount(NucleusOAuthCallbackView)

    expect(postMessageSpy).toHaveBeenCalledWith(
      {
        type: 'nucleus:oauth:callback',
        code: 'auth-code-xyz',
        state: 'csrf-token-abc',
        error: null,
      },
      'http://localhost',
    )
    expect(closeSpy).toHaveBeenCalled()
  })

  it('postMessages error when ?error= present', () => {
    Object.defineProperty(window, 'location', {
      configurable: true,
      writable: true,
      value: {
        ...originalLocation,
        search: '?error=access_denied',
        origin: 'http://localhost',
      },
    })
    Object.defineProperty(window, 'opener', {
      configurable: true,
      writable: true,
      value: { postMessage: postMessageSpy },
    })

    mount(NucleusOAuthCallbackView)

    expect(postMessageSpy).toHaveBeenCalledWith(
      {
        type: 'nucleus:oauth:callback',
        code: null,
        state: null,
        error: 'access_denied',
      },
      'http://localhost',
    )
    expect(closeSpy).toHaveBeenCalled()
  })

  it('closes window even when no opener is present', () => {
    Object.defineProperty(window, 'opener', {
      configurable: true,
      writable: true,
      value: null,
    })

    mount(NucleusOAuthCallbackView)

    expect(postMessageSpy).not.toHaveBeenCalled()
    expect(closeSpy).toHaveBeenCalled()
  })
})
