import { Select } from '../Select.js';
import { useState } from 'react';
import { fitCropRatio, type Crop } from '../../editing-crop.js';
import { Button } from '../components.js';

export function CropTools({
  crop,
  sourceAspect,
  locked,
  onLock,
  onChange,
}: {
  crop: number[];
  sourceAspect: number;
  locked: boolean;
  onLock(value: boolean): void;
  onChange(crop: Crop): void;
}) {
  const [preset, setPreset] = useState('free');
  const [portrait, setPortrait] = useState(sourceAspect < 1);
  const apply = (value: string, vertical: boolean) => {
    setPreset(value);
    onLock(value !== 'free');
    if (value === 'free') return;
    const aspect = value === 'original' ? sourceAspect : Number(value);
    const target = vertical ? Math.min(aspect, 1 / aspect) : Math.max(aspect, 1 / aspect);
    onChange(fitCropRatio(crop as Crop, target / sourceAspect));
  };
  return (
    <div className="grade-crop-tools" role="group" aria-label="裁切比例">
      <Select
        aria-label="裁切比例"
        value={preset}
        onValueChange={(nextValue) => apply(nextValue, portrait)}
      >
        <option value="free">{locked ? '当前比例' : '自由'}</option>
        <option value="original">原始比例</option>
        <option value="1">1 : 1</option>
        <option value="1.25">{portrait ? '4 : 5' : '5 : 4'}</option>
        <option value="1.3333333333333333">{portrait ? '3 : 4' : '4 : 3'}</option>
        <option value="1.5">{portrait ? '2 : 3' : '3 : 2'}</option>
        <option value="1.7777777777777777">{portrait ? '9 : 16' : '16 : 9'}</option>
      </Select>
      <Button
        className="quiet"
        aria-pressed={locked}
        onClick={() => {
          onLock(!locked);
          setPreset('free');
        }}
      >
        {locked ? '比例已锁定' : '锁定比例'}
      </Button>
      <Button
        className="quiet"
        disabled={preset === '1'}
        onClick={() => {
          setPortrait(!portrait);
          if (preset !== 'free') apply(preset, !portrait);
          else {
            onLock(true);
            onChange(
              fitCropRatio(crop as Crop, 1 / ((crop[2]! / crop[3]!) * sourceAspect * sourceAspect))
            );
          }
        }}
      >
        横竖切换
      </Button>
      <Button
        className="quiet"
        onClick={() => {
          onChange([0, 0, 1, 1]);
          setPreset('free');
          onLock(false);
        }}
      >
        重置裁切
      </Button>
    </div>
  );
}
