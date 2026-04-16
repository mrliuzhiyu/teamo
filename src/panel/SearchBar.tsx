import { forwardRef } from "react";

interface Props {
  value: string;
  onChange: (v: string) => void;
  searching: boolean;
}

const SearchBar = forwardRef<HTMLInputElement, Props>(function SearchBar(
  { value, onChange, searching },
  ref,
) {
  return (
    <div className="relative px-3 py-2 border-b border-stone-200 bg-white">
      <input
        ref={ref}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="搜索你的剪切板..."
        className="w-full px-3 py-2 text-sm text-stone-900 bg-stone-50 border border-stone-200 rounded-md outline-none focus:border-stone-400 placeholder:text-stone-400"
        autoFocus
        spellCheck={false}
      />
      {searching && (
        <span className="absolute right-5 top-1/2 -translate-y-1/2 text-[10px] text-stone-400">
          搜索中...
        </span>
      )}
      {value && !searching && (
        <button
          onClick={() => onChange("")}
          className="absolute right-5 top-1/2 -translate-y-1/2 text-xs text-stone-400 hover:text-stone-600"
          tabIndex={-1}
        >
          清除
        </button>
      )}
    </div>
  );
});

export default SearchBar;
