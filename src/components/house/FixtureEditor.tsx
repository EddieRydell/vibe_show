import { useState } from "react";
import type { FixtureDef, BulbShape, PixelType, ChannelOrder } from "../../types";

interface Props {
  fixture: FixtureDef | null; // null = create mode
  onSave: (fixture: FixtureDef) => void;
  onCancel: () => void;
  nextId: number;
}

const BULB_SHAPES: BulbShape[] = ["LED", "C9", "C7", "Mini", "Flood", "Icicle", "Globe", "Snowflake"];
const COLOR_MODELS: FixtureDef["color_model"][] = ["Rgb", "Rgbw", "Single"];
const CHANNEL_ORDERS: ChannelOrder[] = ["Rgb", "Grb", "Brg", "Rbg", "Gbr", "Bgr"];

export function FixtureEditor({ fixture, onSave, onCancel, nextId }: Props) {
  const [name, setName] = useState(fixture?.name ?? "");
  const [colorModel, setColorModel] = useState<FixtureDef["color_model"]>(fixture?.color_model ?? "Rgb");
  const [pixelCount, setPixelCount] = useState(fixture?.pixel_count ?? 50);
  const [pixelType, setPixelType] = useState<PixelType>(fixture?.pixel_type ?? "Smart");
  const [bulbShape, setBulbShape] = useState<BulbShape>(fixture?.bulb_shape ?? "LED");
  const [channelOrder, setChannelOrder] = useState<ChannelOrder>(fixture?.channel_order ?? "Rgb");
  const [radiusOverride, setRadiusOverride] = useState<string>(
    fixture?.display_radius_override != null ? String(fixture.display_radius_override) : ""
  );

  const handleSave = () => {
    if (!name.trim()) return;
    const parsedRadius = radiusOverride.trim() ? parseFloat(radiusOverride) : null;
    onSave({
      id: fixture?.id ?? nextId,
      name: name.trim(),
      color_model: colorModel,
      pixel_count: Math.max(1, pixelCount),
      pixel_type: pixelType,
      bulb_shape: bulbShape,
      channel_order: channelOrder,
      display_radius_override: parsedRadius != null && !isNaN(parsedRadius) ? parsedRadius : null,
    });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-surface border-border w-[420px] rounded-lg border shadow-xl">
        <div className="border-border border-b px-5 py-3">
          <h3 className="text-text text-sm font-bold">
            {fixture ? "Edit Fixture" : "New Fixture"}
          </h3>
        </div>

        <div className="space-y-3 px-5 py-4">
          {/* Name */}
          <label className="block">
            <span className="text-text-2 mb-1 block text-xs">Name</span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Roofline Left"
              autoFocus
              className="border-border bg-surface-2 text-text placeholder:text-text-2 w-full rounded border px-3 py-1.5 text-sm outline-none focus:border-primary"
            />
          </label>

          {/* Color Model + Pixel Count */}
          <div className="flex gap-3">
            <label className="flex-1">
              <span className="text-text-2 mb-1 block text-xs">Color Model</span>
              <select
                value={colorModel}
                onChange={(e) => setColorModel(e.target.value as FixtureDef["color_model"])}
                className="border-border bg-surface-2 text-text w-full rounded border px-2 py-1.5 text-sm outline-none focus:border-primary"
              >
                {COLOR_MODELS.map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
              </select>
            </label>
            <label className="w-24">
              <span className="text-text-2 mb-1 block text-xs">Pixels</span>
              <input
                type="number"
                min={1}
                value={pixelCount}
                onChange={(e) => setPixelCount(parseInt(e.target.value) || 1)}
                className="border-border bg-surface-2 text-text w-full rounded border px-2 py-1.5 text-sm outline-none focus:border-primary"
              />
            </label>
          </div>

          {/* Pixel Type toggle */}
          <label className="block">
            <span className="text-text-2 mb-1 block text-xs">Pixel Type</span>
            <div className="flex gap-1">
              {(["Smart", "Dumb"] as PixelType[]).map((pt) => (
                <button
                  key={pt}
                  onClick={() => setPixelType(pt)}
                  className={`flex-1 rounded px-3 py-1.5 text-xs font-medium transition-colors ${
                    pixelType === pt
                      ? "bg-primary text-white"
                      : "bg-surface-2 text-text-2 hover:text-text border-border border"
                  }`}
                >
                  {pt}
                </button>
              ))}
            </div>
          </label>

          {/* Bulb Shape + Channel Order */}
          <div className="flex gap-3">
            <label className="flex-1">
              <span className="text-text-2 mb-1 block text-xs">Bulb Shape</span>
              <select
                value={bulbShape}
                onChange={(e) => setBulbShape(e.target.value as BulbShape)}
                className="border-border bg-surface-2 text-text w-full rounded border px-2 py-1.5 text-sm outline-none focus:border-primary"
              >
                {BULB_SHAPES.map((s) => (
                  <option key={s} value={s}>{s}</option>
                ))}
              </select>
            </label>
            <label className="flex-1">
              <span className="text-text-2 mb-1 block text-xs">Channel Order</span>
              <select
                value={channelOrder}
                onChange={(e) => setChannelOrder(e.target.value as ChannelOrder)}
                className="border-border bg-surface-2 text-text w-full rounded border px-2 py-1.5 text-sm outline-none focus:border-primary"
              >
                {CHANNEL_ORDERS.map((o) => (
                  <option key={o} value={o}>{o}</option>
                ))}
              </select>
            </label>
          </div>

          {/* Display Size Override */}
          <label className="block">
            <span className="text-text-2 mb-1 block text-xs">Display Size Override (optional)</span>
            <input
              type="number"
              step={0.1}
              min={0.1}
              value={radiusOverride}
              onChange={(e) => setRadiusOverride(e.target.value)}
              placeholder="Auto from bulb shape"
              className="border-border bg-surface-2 text-text placeholder:text-text-2 w-full rounded border px-3 py-1.5 text-sm outline-none focus:border-primary"
            />
          </label>
        </div>

        {/* Actions */}
        <div className="border-border flex justify-end gap-2 border-t px-5 py-3">
          <button
            onClick={onCancel}
            className="text-text-2 hover:text-text rounded px-3 py-1.5 text-xs transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={!name.trim()}
            className="bg-primary hover:bg-primary-hover rounded px-4 py-1.5 text-xs font-medium text-white disabled:opacity-50"
          >
            {fixture ? "Save" : "Create"}
          </button>
        </div>
      </div>
    </div>
  );
}
