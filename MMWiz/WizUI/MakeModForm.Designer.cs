namespace WizUI
{
    partial class MakeModForm
    {
        /// <summary>
        /// Required designer variable.
        /// </summary>
        private System.ComponentModel.IContainer components = null;

        /// <summary>
        /// Clean up any resources being used.
        /// </summary>
        /// <param name="disposing">true if managed resources should be disposed; otherwise, false.</param>
        protected override void Dispose(bool disposing)
        {
            if (disposing && (components != null))
            {
                components.Dispose();
            }
            base.Dispose(disposing);
        }

        #region Component Designer generated code

        /// <summary>
        /// Required method for Designer support - do not modify 
        /// the contents of this method with the code editor.
        /// </summary>
        private void InitializeComponent()
        {
            this.OpenBTN = new System.Windows.Forms.Button();
            this.ModNameLbl = new System.Windows.Forms.Label();
            this.ModNameTB = new System.Windows.Forms.TextBox();
            this.ModDestLbl = new System.Windows.Forms.Label();
            this.TargetDirLbl = new System.Windows.Forms.Label();
            this.CreateButton = new System.Windows.Forms.Button();
            this.SuspendLayout();
            // 
            // OpenBTN
            // 
            this.OpenBTN.Location = new System.Drawing.Point(13, 13);
            this.OpenBTN.Name = "OpenBTN";
            this.OpenBTN.Size = new System.Drawing.Size(75, 23);
            this.OpenBTN.TabIndex = 0;
            this.OpenBTN.Text = "Open File...";
            this.OpenBTN.UseVisualStyleBackColor = true;
            // 
            // ModNameLbl
            // 
            this.ModNameLbl.AutoSize = true;
            this.ModNameLbl.Location = new System.Drawing.Point(13, 43);
            this.ModNameLbl.Name = "ModNameLbl";
            this.ModNameLbl.Size = new System.Drawing.Size(62, 13);
            this.ModNameLbl.TabIndex = 1;
            this.ModNameLbl.Text = "Mod Name:";
            // 
            // ModNameTB
            // 
            this.ModNameTB.Location = new System.Drawing.Point(16, 60);
            this.ModNameTB.Name = "ModNameTB";
            this.ModNameTB.Size = new System.Drawing.Size(152, 20);
            this.ModNameTB.TabIndex = 2;
            // 
            // ModDestLbl
            // 
            this.ModDestLbl.AutoSize = true;
            this.ModDestLbl.Location = new System.Drawing.Point(13, 95);
            this.ModDestLbl.Name = "ModDestLbl";
            this.ModDestLbl.Size = new System.Drawing.Size(113, 13);
            this.ModDestLbl.TabIndex = 3;
            this.ModDestLbl.Text = "Mod will be created in:";
            // 
            // TargetDirLbl
            // 
            this.TargetDirLbl.AutoSize = true;
            this.TargetDirLbl.Location = new System.Drawing.Point(13, 118);
            this.TargetDirLbl.Name = "TargetDirLbl";
            this.TargetDirLbl.Size = new System.Drawing.Size(83, 13);
            this.TargetDirLbl.TabIndex = 4;
            this.TargetDirLbl.Text = "Filled in by code";
            // 
            // CreateButton
            // 
            this.CreateButton.Location = new System.Drawing.Point(13, 146);
            this.CreateButton.Name = "CreateButton";
            this.CreateButton.Size = new System.Drawing.Size(75, 23);
            this.CreateButton.TabIndex = 5;
            this.CreateButton.Text = "Create";
            this.CreateButton.UseVisualStyleBackColor = true;
            // 
            // MakeModForm
            // 
            this.AutoScaleDimensions = new System.Drawing.SizeF(6F, 13F);
            this.AutoScaleMode = System.Windows.Forms.AutoScaleMode.Font;
            this.ClientSize = new System.Drawing.Size(323, 181);
            this.Controls.Add(this.CreateButton);
            this.Controls.Add(this.TargetDirLbl);
            this.Controls.Add(this.ModDestLbl);
            this.Controls.Add(this.ModNameTB);
            this.Controls.Add(this.ModNameLbl);
            this.Controls.Add(this.OpenBTN);
            this.Name = "MakeModForm";
            this.Text = "Make Mod From Snapshot";
            this.ResumeLayout(false);
            this.PerformLayout();

        }

        #endregion

        public System.Windows.Forms.Button OpenBTN;
        private System.Windows.Forms.Label ModNameLbl;
        public System.Windows.Forms.TextBox ModNameTB;
        private System.Windows.Forms.Label ModDestLbl;
        public System.Windows.Forms.Label TargetDirLbl;
        public System.Windows.Forms.Button CreateButton;
    }
}
