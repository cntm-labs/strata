import { describe, it, expect, vi, beforeEach } from 'vitest'

const { NucleusMock, RouterMock } = vi.hoisted(() => {
  const NucleusMock = {
    getToken: vi.fn<() => string | null>(() => null),
    signOut: vi.fn(async () => {
      // simulate localStorage clear
    }),
  }
  const RouterMock = {
    push: vi.fn(),
  }
  return { NucleusMock, RouterMock }
})

vi.mock('@cntm-labs/nucleus-js', () => ({ Nucleus: NucleusMock }))
vi.mock('@/router', () => ({ default: RouterMock }))

const mockFetch = vi.fn()
vi.stubGlobal('fetch', mockFetch)

const { api } = await import('../client')

beforeEach(() => {
  mockFetch.mockReset()
  NucleusMock.getToken.mockReset().mockReturnValue(null)
  NucleusMock.signOut.mockReset().mockResolvedValue(undefined)
  RouterMock.push.mockReset()
})

describe('api client', () => {
  describe('get', () => {
    it('sends GET request with correct headers', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: 'test' }),
      })

      const result = await api.get('/dashboards')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards', {
        headers: { 'Content-Type': 'application/json' },
      })
      expect(result).toEqual({ data: 'test' })
    })
  })

  describe('post', () => {
    it('sends POST with JSON body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ id: '1' }),
      })

      await api.post('/dashboards', { title: 'New' })

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards', {
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
        body: JSON.stringify({ title: 'New' }),
      })
    })

    it('sends POST without body when no data', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({}),
      })

      await api.post('/dashboards/slug/star')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug/star', {
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
        body: undefined,
      })
    })
  })

  describe('put', () => {
    it('sends PUT with JSON body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ id: '1' }),
      })

      await api.put('/dashboards/slug', { title: 'Updated' })

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug', {
        headers: { 'Content-Type': 'application/json' },
        method: 'PUT',
        body: JSON.stringify({ title: 'Updated' }),
      })
    })

    it('sends PUT without body when no data', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({}),
      })

      await api.put('/dashboards/slug')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug', {
        headers: { 'Content-Type': 'application/json' },
        method: 'PUT',
        body: undefined,
      })
    })
  })

  describe('delete', () => {
    it('sends DELETE request', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(null),
      })

      await api.delete('/dashboards/slug')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug', {
        headers: { 'Content-Type': 'application/json' },
        method: 'DELETE',
      })
    })
  })

  describe('error handling', () => {
    it('throws with server error message', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: () => Promise.resolve({ message: 'Not found' }),
      })

      await expect(api.get('/missing')).rejects.toThrow('Not found')
    })

    it('throws with status text when JSON parse fails', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        statusText: 'Internal Server Error',
        json: () => Promise.reject(new Error('invalid json')),
      })

      await expect(api.get('/broken')).rejects.toThrow('Internal Server Error')
    })

    it('throws generic message when no message in error body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 400,
        json: () => Promise.resolve({}),
      })

      await expect(api.get('/bad')).rejects.toThrow('HTTP 400')
    })
  })

  describe('authentication', () => {
    it('attaches Bearer token when Nucleus has a session', async () => {
      NucleusMock.getToken.mockReturnValue('jwt-xyz')

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: 'ok' }),
      })

      await api.get('/dashboards')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards', {
        headers: {
          'Content-Type': 'application/json',
          Authorization: 'Bearer jwt-xyz',
        },
      })
    })

    it('does not attach Authorization header when no token', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: 'ok' }),
      })

      await api.get('/dashboards')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards', {
        headers: { 'Content-Type': 'application/json' },
      })
    })

    it('calls Nucleus.signOut and router.push on 401', async () => {
      NucleusMock.getToken.mockReturnValue('jwt-1')
      mockFetch.mockResolvedValueOnce({ ok: false, status: 401 })

      await expect(api.get('/protected')).rejects.toThrow('Unauthorized')

      expect(NucleusMock.signOut).toHaveBeenCalledTimes(1)
      expect(RouterMock.push).toHaveBeenCalledWith({ name: 'login' })
    })
  })
})
