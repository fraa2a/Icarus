export const ACCENT_PRESETS = ['purple', 'green', 'blue', 'orange', 'rose']
export const ACCENT_COLORS = {
	purple: { dark: '#c78aff', light: '#761ad6', icon: '#a855f7' },
	green: { dark: '#1bd96a', light: '#007a40', icon: '#22c55e' },
	blue: { dark: '#70b5ff', light: '#1464c0', icon: '#3b82f6' },
	orange: { dark: '#ffad66', light: '#a84608', icon: '#f97316' },
	rose: { dark: '#ff8eb0', light: '#b91c50', icon: '#f43f5e' },
}

export function validAccent(value) {
	return ACCENT_PRESETS.includes(value) ? value : 'purple'
}

export function rgb(hex) {
	return [1, 3, 5].map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16))
}

export function contrastRatio(a, b) {
	const luminance = (hex) => {
		const channels = rgb(hex).map((value) => {
			const channel = value / 255
			return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
		})
		return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
	}
	const values = [luminance(a), luminance(b)].sort((x, y) => y - x)
	return (values[0] + 0.05) / (values[1] + 0.05)
}

export function recolorPixels(pixels, color) {
	const target = rgb(color)
	for (let i = 0; i < pixels.length; i += 4) {
		const [r, g, b, alpha] = pixels.slice(i, i + 4)
		const wing = r >= g && g > b * 1.25
		const backdrop = b > g * 1.3 && r > g * 1.3
		if (!alpha || (!wing && !backdrop)) continue
		const shade = Math.max(r, g, b) / 255
		for (let channel = 0; channel < 3; channel++) {
			pixels[i + channel] = Math.round(target[channel] * shade)
		}
	}
	return pixels
}
