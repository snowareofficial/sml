@version v4

{
  __type: "screen"
  __name: "main"
  width: 320
  height: 240
  children: [
    {
      __type: "lv_label"
      __name: "title"
      x: 10
      y: 8
      text: "Hello LVGL"
    }
    {
      __type: "lv_button"
      __name: "btn_ok"
      x: 20
      y: 60
      width: 100
      height: 40
      text: "OK"
      on_click: "on_btn_ok_clicked"
    }
    {
      __type: "lv_slider"
      __name: "vol"
      x: 20
      y: 120
      width: 200
      on_value_changed: "on_vol_changed"
    }
  ]
}
